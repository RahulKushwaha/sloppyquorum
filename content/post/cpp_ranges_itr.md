+++
author = "Rahul Kushwaha"
title = "Convert hasNext style iterator to C++ Range"
date = "2024-06-09"
description = "How to map a Relational Table on a Key-Value Store? "
tags = [
    "c++",
    "ranges"
]
summary = "Key-value stores provide a generic API to store and retrieve values using the key. In this post, we will see how relational tables can be mapped to key-value stores. "
+++

This is a very short article on how to convert a hasNext style iterator to a ranges object. Imagine we have an iterator of the following form:

```
struct Iterator {
  std::uint64_t next() {
    return (seed++);
  }

  bool hasNext() const {
    return seed < limit;
  }

 private:
  std::uint64_t seed{0};
  std::uint64_t limit{50};
};
```

The above iterator generates numbers until we reach the limit. The usual way to access the elements is to call hasNext , which checks if there are more elements, and based on its value call next to fetch the next element. A simple function shown below describes the pattern.

```
void fun() {
  Iterator itr;
  while (itr.hasNext()) {
    std::cout << itr.next() << "\n"; // Do something with element.
  }
}
```


One of the many ways to convert this iterator to ranges is to use ranges::view::generate method.

```
std::uint64_t seed{0};
auto infiniteRange = ranges::views::generate([seed]() mutable {
  return seed++;
});
```
The code shown above captures seed via copy and keeps on generating number lazily. It represents an infinite sequence of lazily generated numbers.

Now we can augment the above range to generate only n elements by using takeWhile . The code shown below generates 5 elements when iterated over(remember all the elements are created lazily).
```
std::uint64_t seed{0};
auto only5Elements =
    ranges::views::generate([seed]() mutable { return seed++; }) |
    ranges::views::take_while([](auto x) { return x < 5; });
```
Combining the above techniques we can convert a hasNext style iterator to a range using the code shown below.

```
Iterator itr{};
ranges::views::generate([itr]() mutable -> std::optional<uint64_t> {
  // Generate optional of values until hasNext is true.
  if (itr.hasNext()) {
    return itr.next();
  }

  // Otherwise generate empty optionals.
  return std::nullopt;
}) |
    // take_while makes sure that we are taking the elements
    // only until optional is not empty.
    ranges::views::take_while(
        [](auto element) -> bool { return element.has_value(); }) |
    // Extract the value from the optional using transform.
    ranges::views::transform([](auto element) {
      return element.value();
    });
```
The above code can be understood via the following 3 steps:

Create an infinite range of optional<element> using generate . When the underlying iterator does not have any element, hasNext is false, generator will start emitting empty optionals.
Augment the above range to continue iterating over the elements while the optional contains a value. Otherwise, stop. Another way to look at this is to think empty optional as a stopping condition. A sentinel value.
Unwrap the value from optional.
Using the above pattern we can convert a simple hasNext style iterator to a range.

Further readings:

1. https://www.walletfox.com/course/quickref_range_v3.php
2. https://ericniebler.github.io/range-v3/index.html
