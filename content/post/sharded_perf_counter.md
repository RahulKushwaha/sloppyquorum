+++
author = "Rahul Kushwaha"
title = "Killing Cache-Line Contention with Sharded Counters"
date = "2026-05-17"
description = "How a single atomic counter becomes a bottleneck under concurrent writes, and how cache-line-padded shards make it disappear."
tags = [
    "rust",
    "concurrency",
    "performance",
    "atomics",
]
summary = "A single hot AtomicU64 forces every writer to bounce the same cache line between cores. Sharding the counter across cache-line-padded atomics turns a contended fetch_add into uncontended local writes, at the cost of an O(SHARDS) read."
+++

Telemetry counters look harmless. You wrap an `AtomicU64`, sprinkle `fetch_add(1, Relaxed)` through the hot path, and call it a day. Under low concurrency it costs nothing. Under high concurrency, that single counter becomes one of the most expensive lines of code in the program.

The reason is hardware. Every `fetch_add` on the same atomic must acquire that cache line in the modified state on the issuing core, which means evicting it from whichever core had it last. With N cores writing the same counter, you're serializing the cache line through the coherence protocol. The atomic itself is lock-free; the cache line is not.

## The baseline

Here's the naive version most code ships with.

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct Counter(AtomicU64);

impl Counter {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    #[inline]
    pub fn add(&self, value: u64) {
        self.0.fetch_add(value, Ordering::Relaxed);
    }

    pub fn value(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}
```

Eight lines and correct. The read is trivially atomic (a single 8-byte load). The write is `lock xadd` on x86, an LL/SC loop on ARM. The cost on a single thread is a handful of cycles.

Now put it on the hot path of eight worker threads each doing a million increments. The single cache line holding `self.0` has to migrate to whichever core is currently executing `fetch_add`. Every other core that wants to write has to wait for cache coherence to invalidate its copy, transfer the line, and let it acquire exclusive ownership. The atomic operation itself is still lock-free in the textbook sense, but the cache line acts as a lock, and the protocol that moves it is the contention.

You can watch this happen in `perf stat`. `cache-misses` climbs, IPC drops, and adding more threads makes the counter *slower*, not faster. This is the false-sharing-of-one problem.

The fix is to stop sharing the line. Spread writes across many independent atomics, each on its own cache line, and sum them when somebody actually reads.

## The shard

```rust
#[repr(align(128))]
struct Shard(AtomicU64);
```

That's the entire data structure. The `align(128)` is the only thing that matters. It guarantees adjacent shards live on different cache lines. Why 128 and not 64? Apple Silicon uses 128-byte lines. Intel's lines are 64 bytes, but the adjacent-line prefetcher will pull in the neighbor on access, effectively coupling pairs of lines. Padding to 128 bytes covers both. You overpay by one line per shard. At 64 shards that's 8 KiB total. Cheap.

## The write path

```rust
pub fn add(&self, value: u64) {
    let idx = self.indexer.index() % SHARDS;
    self.shards[idx].0.fetch_add(value, Ordering::Relaxed);
}
```

Two things to notice. First, `Relaxed` ordering. The counter doesn't synchronize anything beyond itself. A torn read between two shards is fine because the reader isn't taking a snapshot anyway (more on that in a moment). Pay only for the atomicity you actually need.

Second, the indexer is a trait. The shard for a given write is chosen by an `Indexer` implementation, which is on the write fast path and must be inlinable and lock-free.

```rust
pub trait Indexer {
    fn index(&self) -> usize;
}
```

Two implementations are useful in practice.

### Thread-id indexer (portable)

```rust
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ThreadIdIndexer;

static NEXT_THREAD_SHARD_ID: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static THREAD_SHARD_ID: Cell<usize> = const { Cell::new(usize::MAX) };
}

impl Indexer for ThreadIdIndexer {
    fn index(&self) -> usize {
        THREAD_SHARD_ID.with(|c| {
            let v = c.get();
            if v != usize::MAX {
                return v;            // fast path: one TLS load
            }
            let new_v = NEXT_THREAD_SHARD_ID.fetch_add(1, Ordering::Relaxed);
            c.set(new_v);
            new_v
        })
    }
}
```

Each thread gets a sequential id on first use, cached in a thread-local. The fast path is one TLS load. The slow path, taken exactly once per thread, is one atomic increment of a process-wide counter.

Sequential ids beat hashing the `ThreadId`. The first `SHARDS` threads get perfect distribution, and collisions only start after wraparound. A hash-based scheme can collide on the very first two threads if it's unlucky. The all-ones bit pattern is the unassigned sentinel, which works because no real id ever reaches it.

This implementation is portable. macOS, Linux, Windows, BSDs all give you thread-locals and atomics.

### CPU-id indexer (Linux only, NUMA-aware)

```rust
#[cfg(target_os = "linux")]
pub struct CpuIndexer;

#[cfg(target_os = "linux")]
impl Indexer for CpuIndexer {
    fn index(&self) -> usize {
        let cpu = unsafe { libc::sched_getcpu() };
        if cpu < 0 { 0 } else { cpu as usize }
    }
}
```

The whole implementation is one syscall-like call. `sched_getcpu` returns the CPU the calling thread is currently scheduled on; on modern glibc it's a vDSO call, so it never enters the kernel. It's a few cycles.

The point isn't really to dodge collisions. It's locality. When the kernel migrates a thread to a new core, writes immediately start landing in the shard whose cache line is already hot in that core's L1. With a thread-id indexer the migrated thread keeps writing to its old shard, whose cache line is now stale on the new core and has to be pulled across the interconnect on the very next `fetch_add`. On NUMA boxes or many-core systems, that one extra transfer per migration is exactly the cost the sharding was supposed to eliminate.

The error handling is deliberately weak. On `sched_getcpu` failure (rare; `EFAULT` only, per the man page) it falls back to shard 0 silently. Telemetry is best-effort, not load-bearing; an unhandled error here would be worse than a silent miscount.

Pick `CpuIndexer` on Linux when you can. Fall back to `ThreadIdIndexer` everywhere else. A small `cfg` alias lets the rest of the code stay platform-agnostic.

```rust
#[cfg(target_os = "linux")]
pub type PerfCounter<const N: usize> = ShardedCounter<N, CpuIndexer>;

#[cfg(not(target_os = "linux"))]
pub type PerfCounter<const N: usize> = ShardedCounter<N, ThreadIdIndexer>;
```

## The read path

```rust
pub fn value(&self) -> u64 {
    let mut total: u64 = 0;
    for s in &self.shards {
        total = total.wrapping_add(s.0.load(Ordering::Relaxed));
    }
    total
}
```

O(SHARDS), and explicitly **not a snapshot**. Shards are loaded one at a time with `Relaxed` ordering, so concurrent writers can land in a shard that's already been read, or in one that hasn't been read yet. The return value lies somewhere between the counter's value at the start of the call and its value at the end.

For telemetry this is fine. You're going to graph the number, not branch on it. The contract is explicit. It's fine for telemetry but not for anything you want to branch on. If you violate that contract you have nobody to blame.

`reset()` has the same caveat. A concurrent writer can land in a shard that's already been zeroed, leaving the counter non-zero immediately after `reset` returns. Quiesce writers first or accept the drift.

## Picking `SHARDS`

A reasonable rule of thumb is the smallest power of two that's `>=` the number of concurrent writers you expect. Power of two so the `% SHARDS` lowers to a bitmask. Larger reduces contention; smaller keeps reads fast and the working set small. Each shard costs 128 bytes, so 64 shards is 8 KiB and 256 shards is 32 KiB. The sweet spot for a server-class workload is usually 32 to 64.

## Measured

Theory is one thing; numbers are another. I ran a small benchmark that
contends N worker threads against (a) a single `AtomicU64` and (b) a
64-shard `ShardedCounter` with a thread-id indexer, each thread doing
5 million `fetch_add(1, Relaxed)` operations.

Run on an AMD Ryzen Threadripper 3970X (32 physical cores, 64
threads with SMT), 192 GiB DDR4, Ubuntu 25.10, Linux 6.17, rustc
1.93.1, release build with `lto = true` and `codegen-units = 1`. Best
of three runs reported.

| Threads | Naive (Mops/s) | Sharded (Mops/s) | Speedup |
|--------:|---------------:|-----------------:|--------:|
|       1 |         105.94 |           108.56 |    1.0x |
|       2 |          53.44 |           216.79 |    4.1x |
|       4 |          47.78 |           433.25 |    9.1x |
|       8 |          46.82 |           861.90 |   18.4x |
|      16 |          46.63 |          1714.33 |   36.8x |
|      32 |          47.48 |          3336.20 |   70.3x |
|      64 |          54.89 |          6136.46 |  111.8x |

Three things jump out.

**Adding a second thread halves the naive counter's throughput.** It
goes from 106 Mops/s to 53 Mops/s. That isn't "scaling poorly"; it's
the cache line transitioning from a single owner doing uncontended
`xadd` to a coherence ping-pong between two cores. Every increment
now waits for the line to migrate.

**Past two threads, naive throughput is flat.** 47 Mops/s at 4 cores,
at 16 cores, at 32 cores. The bottleneck is the cache coherence
protocol, and adding cores doesn't help; it just adds more contenders
for the same line. The 64-thread row creeps up to 55 Mops/s because
SMT siblings share L1 and the line doesn't have to leave the core for
intra-core contention.

**Sharded scales nearly linearly.** Doubling the threads roughly
doubles the throughput, all the way to 6.1 Gops/s at 64 threads. At
32 physical cores the sharded counter is **70x faster** than the
naive one. At 64 threads (with SMT) it's almost 112x. That ratio is
not subtle.

Single-threaded performance is essentially identical between the two
(106 vs 109 Mops/s). The thread-local indexer adds one TLS load per
increment, which on this CPU is roughly free. So there's no
fast-path tax to pay for the slow-path win.

**The speedup column exceeds the thread count, but the math is honest.**
Sharded scales at most linearly with cores. The 32-thread row is
31.5x the single-thread baseline (3336 / 106), exactly the ceiling
you'd expect from 32 cores. The reason the reported speedup is 70x
and not 32x is that naive doesn't just fail to scale; it *regresses*.
A single thread does 106 Mops/s; 32 threads competing on the same
line manage 47 Mops/s, a 2.25x slowdown from the uncontended
baseline. The speedup column is `sharded(N) / naive(N)`, so it
multiplies the two effects. 31.5x scaling on the numerator times
2.25x degradation on the denominator gives 70x. The cleaner framing
isn't "sharded is 70x faster"; it's "sharded scales linearly and
naive moves backwards, so the gap compounds."

## What this pattern is and isn't

This pattern is a way to amortize the cost of an atomic write across many cache lines so writers never wait on each other.

What it isn't is a snapshot counter, a histogram, a precise meter for billing or rate limiting, or anything you want to compare against a threshold. The relaxed semantics give you throughput, not correctness, and the read aggregation gives you an estimate, not a value.

The right rule of thumb is simple. If losing or double-counting a few increments under contention would cause a bug, don't use this. If you're going to feed the number into a dashboard, this is the cheapest correct thing you can do.
