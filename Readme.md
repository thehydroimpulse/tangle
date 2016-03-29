# Tangle

[![Build Status](https://travis-ci.org/thehydroimpulse/tangle.svg?branch=master)](https://travis-ci.org/thehydroimpulse/tangle)

[Documentation (Master)](http://thehydroimpulse.github.io/tangle)

Futures implementation in Rust based on Scala's Futures that runs in a thread pool. It allows composable asynchronous concurrency in a way that works nicely with existing Rust constructs.

## Getting Started

Install tangle with Cargo:

```toml
[dependencies]
tangle = "0.4.0"
```

Add the crate to your project:

```rust
extern crate tangle;
```

And require the only two types you need from this crate:

```rust
use tangle::{Future, Async};
```

## Creating a Future

**Using Values**

The first way to create a future is through a unit, or an already resolved value. This will not require any threading and allows you to lift a value into a Future, making it composable.

```rust
Future::unit(1);
Future::unit("hello world".to_string());
```

Future values must implement the `Send` trait regardless if threading is involved.

**Using Closures**

You may also create a Future through the use of a closure. The closure is expected to return the `Async<T, E>` type which is an asynchronous version of `Result<T, E>`.

```rust
Future::new(move || {
  let result = // perform some heavy work here...

  Async::Ok(result);
});
```

**Using Channels**

Channels are essentially the substitute to promises. Usually, promises are used for writing and futures are used for reading; however, Tangle replaces the writing part with Rust channels.

You may let Tangle create the channel or you may pass the receiver-end yourself, using `::channel` and `::from_channel`, respectively.

```rust
let (tx, future) = Future::channel();

tx.send(123);
```

Using an existing channel:

```rust
use std::sync::mpsc::channel;

let (tx, rx) = channel();
let future = Future::from_channel(rx);

tx.send(123);
```

## Resolving Futures

The point is to eventually get some sort of value back from the futures. You may either block on the future using the `.recv()` method, or you may chain using `and_then` (flat map) and `map`. The latter methods are all asynchronous and continue running in a thread pool, they also themselves return new futures.

### Blocking

```rust
let future = Future::unit(123);

// recv() returns `Result<T, E>`
assert_eq!(future.recv().unwrap(), 123);
```

### `and_then`

You need to wrap the value back into an `Async` type.

```rust
let future = Future::unit(123);

future.and_then(move |n| {
  Async::Ok(n * 100)
});
```

You may also dynamically compose futures using `Async::Continue`.

```rust
let future = Future::unit(123);

future.and_then(move |n| {
  // ...
  Ok::Continue(find_user_id("thehydroimpulse"))
});

fn find_user_id(name: &str) -> Future<u64> {
  // ...
  return Async::Ok(...);
}
```

### `map`

```rust
let future = Future::unit(123);

future.map(move |n| n * 100);
```

## Error Handling

TODO: Write documentation.

## License

The MIT License (MIT)
Copyright (c) 2016 Daniel Fagnan <dnfagnan@gmail.com>

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
