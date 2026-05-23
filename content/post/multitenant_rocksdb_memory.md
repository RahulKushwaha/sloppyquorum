+++
author = "Rahul Kushwaha"
title = "Block Cache, Page Cache, and Multi-Tenant Memory Density"
date = "2026-05-22"
description = "Multi-tenant RocksDB has two ways to share host memory. One trades latency for elasticity. The other needs an allocator you control."
tags = [
    "rocksdb",
    "database",
    "storage",
    "memory",
    "rust",
]
summary = "If every tenant on a host runs its own RocksDB, you have to pick how their memory is shared. The page-cache route lets the OS arbitrate but charges decrypt and decompress on every read. The block-cache route is dramatically faster (37x on point reads against a 10 GB working set with a 1 GiB cache, in the run below) but pushes the arbitration problem back onto you, which is why the first non-trivial piece of the design ends up being a custom allocator."
+++

Multi-tenant storage planes tend to converge on the same shape. Each customer gets their own LSM (a RocksDB instance, in this case) so blast radius stays small, snapshots are independent, and one tenant's compaction backlog can't pin the threads another tenant's reads need. You run many of these per host, hundreds in some shops, thousands in others.

The interesting question is then a memory question. The host has, say, 256 GiB of RAM. There are 500 tenants. Memory is the limit on density. How do you share it?

There are two reasonable answers and they have very different consequences. I spent today building both and measuring them against each other on an encrypted RocksDB, and the numbers turned out cleaner than I expected.

## The two strategies

**Strategy A. Lean on the page cache.** Each tenant gets a deliberately tiny block cache (8 MiB, say) and the OS page cache takes most of the working set. Blocks that aren't currently being read live as `file-backed mmap` pages, and the kernel evicts whichever pages it wants when memory gets tight. You don't have to manage anything. That's the whole appeal. A quiet tenant naturally relinquishes memory because nobody is touching its pages, so the kernel pushes them out.

**Strategy B. Lean on the block cache.** Each tenant gets a big block cache (256 MiB, say) and you turn on direct I/O so the same blocks don't end up sitting in both RocksDB's block cache and the kernel page cache. The cache holds fully-decoded blocks, decompressed and decrypted, ready to read. Reads are pointer dereferences. The problem moves to *you*. You now have to decide which tenant gets how much memory and how to take it back when allocations need to shift.

The first is the path of least resistance. The second is the path of low latency. The gap between them, as it turns out, is large.

## Why the page-cache route hurts

It's tempting to think the page cache "is just memory" and so a hit there should be as fast as a hit in the block cache. It is not, for one structural reason. Anything in the page cache is in its on-disk form.

A RocksDB block on disk looks like this, end to end.

```
encrypted( compressed( block_format( key/value pairs ) ) )
```

Compression has to run before encryption, not the other way around. AES-CTR ciphertext is statistically indistinguishable from random bytes, and random bytes don't compress, so compressing the output of encryption would waste CPU for no size win. RocksDB compresses inside the block-based table layer. EncryptedEnv sits underneath at the file I/O layer, so it sees the already-compressed bytes and encrypts those. Every read from the page cache has to do the following.

1. Copy the page from kernel space to user space.
2. AES-CTR decrypt it (or whatever encryption sits at the env layer).
3. Decompress it (LZ4, zstd, snappy, whatever you've picked).
4. Parse the block format to find the right entry.
5. Hand a pointer to the caller.

Steps 2 through 4 are not optional. The page cache has the bytes. It doesn't have the *block*. To get the block you pay the full cold path, minus the disk read.

The block cache, by contrast, caches the *block*. Once it's in there, a read is just a hash lookup and a memcpy into a `PinnableSlice`. Decryption ran exactly once, when the block was loaded. Decompression ran exactly once. Each subsequent hit skips all of it.

The difference is the cost of those steps times your read rate. Per block, on a modern CPU, you're looking at single-digit microseconds for AES-CTR on a 4 KiB block plus a few more for LZ4. That's nothing in isolation. At 100k reads per second, it's everything.

## Set the table

To measure this with real numbers, I needed an encrypted RocksDB with two configurations.

1. **Page-cache regime.** 8 MiB block cache, buffered I/O. Working set lives in the OS page cache.
2. **Block-cache regime.** 1 GiB block cache, direct I/O on for reads and compaction. Working set still doesn't fit. The cache holds about 10%.

Encryption is AES-256-CTR via RocksDB's `EncryptedEnv`, with a `BlockCipher` subclass that delegates 16-byte counter encryption to the Rust `aes` crate. CTR is a stream cipher, so there's no alignment problem. The per-file 4 KiB IV prefix preserves direct-I/O alignment for the rest of the file. SST, WAL, MANIFEST, CURRENT, IDENTITY, and OPTIONS are all encrypted in 10.x. Only the info LOG goes through a separate `Logger` path and stays plaintext.

The dataset is 10 million keys with 1 KiB values (~10 GiB on disk). I chose this size deliberately. It's the multi-tenant case where neither strategy gets to be comfortable. The page-cache regime can't reliably keep 10 GB hot against the rest of the workstation's memory pressure, and the 1 GiB block cache only holds ~10% of the working set, so the block-cache regime also evicts. The point of the comparison is what each strategy does when its cache is smaller than what's being asked of it. The workload is 100k uniformly-random point reads followed by 1k range scans of 100 entries each.

Hardware was an AMD Ryzen Threadripper 3970X with 192 GiB DDR4, NVMe SSD, on Linux 6.17. Release build with debug symbols.

## Numbers

Point reads, with latencies in microseconds.

| Regime                   | qps   |   p50 |   p99 | p99.9 |  p100 |
|--------------------------|------:|------:|------:|------:|------:|
| Page cache (8 MiB)       |   160 |  6234 |  6705 |  7589 |  9920 |
| Block cache (1 GiB, DIO) | 5,911 |   183 |   233 |   280 |  1264 |

Range scans (1000 scans of 100 entries each), again in microseconds.

| Regime                   | scans/s |   p50 |   p99 | p99.9 |  p100 |
|--------------------------|--------:|------:|------:|------:|------:|
| Page cache (8 MiB)       |     119 |  8347 |  9101 |  9445 | 10158 |
| Block cache (1 GiB, DIO) |     312 |  3323 |  4155 |  4325 |  4669 |

The point-read throughput ratio is **37x**. The p50 ratio is 34x. The scan ratio drops to 2.6x. The shape is worth pausing on. At this scale, both regimes are missing their cache on most reads, so the gap isn't "cached vs uncached" anymore. It's about what a cache miss costs in each regime.

Scans equalize because once you're already paying for I/O, the cost of pulling 100 entries out of a block dwarfs whether the block came from a hot cache or a warm one. The 2.6x gap that remains is the per-block decode work that the block cache amortizes across all 100 entries (one decrypt+decompress, then 100 lookups) but the page cache regime pays once per block-load.

Point reads, on the other hand, get worse at this scale, not better. The block-cache regime is now 183 us per read (not 3 us), but the page-cache regime is 6.2 ms per read (not 72 us). The absolute floor moved up for both, but the page-cache regime moved up much further.

The p100 column is the most interesting addition. The block-cache regime has a fat tail relative to its median, with p100/p50 around 6.9x. The page-cache regime is the opposite, with p100/p50 around 1.6x. That's not a quirk. It's the same mechanism that gives the block cache its low median showing up in reverse at the tail. When most reads land in cache and finish in tens of microseconds, the few that miss several blocks at once stand out. When every read is doing the full pipeline anyway, there's nothing left for the tail to do that the median wasn't already doing. The page-cache distribution is flat because it has no fast path to fall off of.

Even so, the block cache's *worst* read (1264 us) is 7.8x faster than the page cache's *median* read. The tail is fatter, but the tail is still well below the floor of the other strategy. That's the multi-tenant case in one number.

## Where the latency goes

A point read in RocksDB doesn't touch one block. It walks three. A bloom filter block, then an index block, then the data block. When the cache has room for all three categories the walk is fast. When it doesn't, each category becomes its own miss path.

The 8 MiB cache can't hold a meaningful fraction of the filter and index blocks for a 10 GiB SST footprint, so they thrash alongside data blocks. A logical point read becomes two or three physical block loads, each paying the full buffered-read plus decode pipeline. That's where 6234 us per point read comes from. It's not "one disk read" worth of work. It's two or three.

The 1 GiB block-cache regime walks the same three blocks but holds them differently. Filter and index blocks are pinned (`pin_l0_filter_and_index_blocks_in_cache(true)`, `cache_index_and_filter_blocks(true)`), so those two lookups always hit. The data block has a ~10% chance of being cached. When it is, the read finishes in a handful of microseconds. When it isn't, the read costs one DirectIO request plus one decode. About 10% of reads at near-zero cost and 90% at one disk read plus one decode average to the 183 us p50.

Put it in one sentence. A block cache big enough to pin the filter and index blocks drops the per-read amplification from 2-3 misses to 0-1, and that single saved miss is most of the gap.

(In an earlier run with a working set that fit (110 MiB data, 256 MiB cache), the allocator counters reported 27,782 allocations and 2 frees over 100k reads. The same workload through the 8 MiB cache reported 124,775 allocations and 122,947 frees. The eviction story those counters tell is exactly what shows up in latency.)

## So why not just use the block cache for everything

Because the easy thing about the page-cache strategy is the only thing that matters. The OS gives memory back.

If tenant 47 went quiet at 3 AM, the kernel will, on its own, push tenant 47's blocks out of the page cache to make room for tenant 12's blocks at 4 AM. You don't have to do anything. You don't have to know that tenant 47 is quiet. You don't even have to know tenant 47 exists. The kernel's LRU and the access pattern do the work.

The block cache will not do this for you. A `LRUCache` in RocksDB grows to its configured capacity and stays there. Configure 256 MiB and you've committed 256 MiB to that tenant until the process exits. If you run 500 tenants like that, you've committed 125 GiB. The capacity is yours to manage.

You can resize block caches at runtime. RocksDB supports `SetCapacity` on the cache. But "shrinking" an LRU cache just means it stops growing and evicts entries until it's under the new capacity. The memory those entries occupied is `free()`d, and what happens after that is up to the allocator. Even with jemalloc, which is more aggressive about returning unused pages to the kernel than most other general-purpose allocators, you don't get a primitive that says "release this 1 GiB now and update my RSS." You get reclaim that *might* happen, on a schedule you don't control, at a granularity you can't dictate.

This is the part that bites you. You can tell the cache to release 1 GiB. The cache will dutifully evict entries until 1 GiB worth of allocations have been freed. The allocator will hold onto those bytes in its arenas until it sees fit. From the OS's perspective, your process still owns the same RSS. The memory you thought you reclaimed is gone from the cache and unavailable to the next tenant who needs it. You ratchet up. You don't ratchet down.

`MADV_DONTNEED` on heap pages sometimes works. Arena-specific reclaim hooks sometimes work. None of this is reliable across allocator versions, build flags, and fragmentation states. It's a folk-medicine approach to a problem that wants a real fix.

## The real fix is owning the allocator

If the block cache calls into an allocator *you control*, you can make "give the memory back to the OS" a routine operation rather than an act of faith. The straightforward shape is `mmap`-backed allocations. Each cache entry's bytes come from an `mmap(MAP_ANON | MAP_PRIVATE)` region, and `Deallocate` calls `munmap` on it. `munmap` is not advisory. It removes the mapping from the process's address space, the kernel takes the pages back, RSS drops, and another process (or another tenant's allocator) can have them.

The cost of going through `mmap`/`munmap` per allocation is real (system call overhead, address space fragmentation, TLB shootdowns on multi-threaded workloads), so a production version probably wants a slab on top. You `mmap` a large region, hand out fixed-size cache-block-sized slots from it, and free a slab back to the OS only when it's entirely empty. That's an allocator design, not a wrapper.

But before you can build any of that, the cache has to be willing to call into your allocator at all. RocksDB exposes this through `MemoryAllocator`.

```cpp
class MemoryAllocator : public Customizable {
 public:
  virtual void* Allocate(size_t size) = 0;
  virtual void Deallocate(void* p) = 0;
};
```

You subclass it and pass an instance through `LRUCacheOptions::memory_allocator` when constructing the cache. Every block-cache entry allocation now goes through your hooks. The C API exposes this as `rocksdb_lru_cache_options_set_memory_allocator`.

The first version doesn't have to do anything clever. Step 1 is just `Allocate` calls `std::alloc::alloc`, `Deallocate` calls `std::alloc::dealloc`. That's what the experiment above measured. It's not faster than the default. The value is that the cache is now allocating through a path you can swap out. Step 2 replaces the body of `Allocate` and `Deallocate` with `mmap` and `munmap` without touching the C++ shim, the cache options, or the bench.

The overhead of going through the Rust callback rather than `new/delete` is sub-noise. On the smaller-dataset run (110 MiB data, 256 MiB cache, working set fits) the comparison was clean.

| Regime                              | POINT qps | POINT p50/p99 |
|-------------------------------------|----------:|--------------:|
| Block cache, default allocator      |   256,014 |   3 / 4 us    |
| Block cache, Rust global allocator  |   244,316 |   3 / 5 us    |

The 4.6% delta is at run-to-run-noise level. The Rust callback plus a 16-byte size header for `Deallocate` costs nothing measurable. RocksDB's interface only hands you a pointer at free time, so the size has to live somewhere, and a header on the allocation is the boring choice. The whole point of measuring this on the small dataset is to isolate the allocator change from cache-pressure effects. At the 10 GB scale the per-read latency is dominated by I/O and decode, so the allocator overhead would be invisible anyway.

## The shape this points to

The architecture that falls out of this looks roughly like the following.

- Each tenant has its own RocksDB instance with its own LRU block cache.
- Every block cache shares one process-wide allocator.
- The allocator is `mmap`-backed at a coarse granularity, with slab logic for cache-block-sized requests.
- A control plane (a separate thread, or external signal) can tell the allocator "release N MiB" and the allocator can `munmap` whole slabs to do it.
- Tenants whose caches are evicted under pressure see slightly higher cold-read latency but their working sets re-warm into freshly-allocated slabs as soon as traffic picks up.

Compared to the page-cache strategy, you keep the 37x point-read latency win at realistic multi-tenant cache sizes. Compared to the naive block-cache strategy, you can actually run hundreds of tenants on the same host without your RSS becoming a one-way function. The cost is one custom allocator, which is roughly the kind of code you write once and then leave alone for a year.

## What this is and isn't

It is an argument for owning the allocator under your block cache when you're running many small instances on a shared host. The latency case for the block cache is overwhelming on encrypted, compressed workloads, and the only thing standing between "block cache wins" and "block cache scales" is the question of who controls the memory.

It is not an argument against the page cache for everything. If your tenants are large enough that each one gets a server, or your blocks aren't encrypted and compressed (in which case the cost of "decode on read" collapses and the gap narrows), or your access pattern is sequential enough that scans dominate, the calculus changes. The point-read-heavy, encrypted, multi-tenant case is where this shape pays off most clearly.

It is also not the final allocator design. Step 1 (route the cache through Rust `std::alloc`) is built and measured. Step 2 (replace the body with `mmap`/`munmap`) is the next thing. The shim is wired so the bench harness, the C++ side, and the cache construction code don't need to change when that swap happens. That is, I think, the most useful thing you get out of doing this in two steps instead of one.

The next few posts in this series will walk through building that allocator. It'll be a slab on top of `mmap`-backed regions, with `munmap` as the "give it back" primitive, sized for cache-block-sized allocations, and instrumented well enough that you can actually see RSS drop when a tenant goes quiet. The interesting parts will be the address-space layout, the slab metadata, fragmentation under churn, and the cost of `munmap` (TLB shootdowns are real and not free). The bench you saw here will become the regression harness for it.
