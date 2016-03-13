//! A lightweight futures package inspired by Scala. The goals are to provide
//! a simple interface for creating futures and, most importantly, composing multiple
//! asynchronous actions together. Thus, all futures return `Async<T, E>` which is
//! an asynchronous equivalent to `Result<T, E>`, the only difference being that
//! an extra variant `Continue(Future<T, E>)` allows for composition.

use std::thread;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Mutex;
use std::convert;
use std::mem;
use threadpool::ThreadPool;

pub use Async::Continue;

#[macro_use]
extern crate lazy_static;
extern crate threadpool;
extern crate num_cpus;

lazy_static! {
    static ref POOL: Mutex<ThreadPool> = Mutex::new(ThreadPool::new(num_cpus::get()));
}

/// Asynchronous version of `Result<T, E>` that allows for future composition. Additional
/// macros are provided to work with both `Async<T, E>` and `Result<T, E>`.
#[derive(Debug)]
pub enum Async<T, E> {
    Ok(T),
    Err(E),
    Continue(Future<T, E>)
}

impl<T, E> Async<T, E> {
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Async<U, E> {
        match self {
            Async::Ok(t) => Async::Ok(f(t)),
            Async::Err(e) => Async::Err(e),
            Async::Continue(cont) => panic!("Cannot map on `Async::Continue`. Use `map_future` for that.")
        }
    }

    pub fn map_err<U, F: FnOnce(E) -> U>(self, f: F) -> Async<T, U> {
        match self {
            Async::Ok(t) => Async::Ok(t),
            Async::Err(e) => Async::Err(f(e)),
            Async::Continue(cont) => panic!("Cannot map on `Async::Continue`. Use `map_future` for that.")
        }
    }

    pub fn is_err(&self) -> bool {
        match self {
            &Async::Err(_) => true,
            _ => false
        }
    }

    pub fn is_future(&self) -> bool {
        match self {
            &Async::Continue(_) => true,
            _ => false
        }
    }

    pub fn is_ok(&self) -> bool {
        match self {
            &Async::Ok(_) => true,
            _ => false
        }
    }
}

/// ok!(123)
#[macro_export]
macro_rules! ok {
    ($expr:expr) => (Async::Ok($expr))
}

// err!(123)
#[macro_export]
macro_rules! err {
    ($expr:expr) => (Async::Err($expr))
}

/// compose!(future! {
///     err!(123)
/// })
#[macro_export]
macro_rules! compose {
    ($expr:expr) => (Async::Err($expr))
}

#[macro_export]
macro_rules! async {
    ($expr:expr) => (match $expr {
        Result::Ok(val) => val,
        Result::Err(err) => {
            use std::convert;
            return $crate::Async::Err(convert::From::from(err));
        }
    })
}

/// Create a `Future` with a slightly nicer syntax.
///
/// ```notrust
/// future! {
///     Ok(123)
/// }
/// ```
#[macro_export]
macro_rules! future {
    ($x:expr) => {{
        Future::new(|| {
            $x
        })
    }}
}

#[derive(Debug)]
pub enum PromiseState {
    Waiting,
    Resolved,
    Failed
}

#[derive(Debug)]
pub struct Promise<T, E=()> {
    chan: Sender<Async<T, E>>,
    rx: Option<Receiver<Async<T, E>>>,
    state: PromiseState
}

impl<T, E> Promise<T, E>
    where T: Send + 'static,
          E: Send + 'static
{
    /// ```
    /// use tangle::{Promise};
    /// let mut p = Promise::<u32, ()>::new();
    ///
    /// p.future();
    /// ```
    pub fn new() -> Promise<T, E> {
        let (tx, rx) = channel::<Async<T, E>>();

        Promise {
            chan: tx,
            rx: Some(rx),
            state: PromiseState::Waiting
        }
    }

    pub fn future(&mut self) -> Future<T, E> {
        if let Some(rx) = mem::replace(&mut self.rx, None) {
            return Future::<T, E>::from_async_channel(rx);
        } else {
            panic!("Unexpected None");
        }
    }
}

/// A value that will be resolved sometime into the future, asynchronously. `Future`s use
/// an internal threadpool to handle asynchronous tasks.
#[derive(Debug)]
pub struct Future<T, E=()> {
    receiver: Receiver<Async<T, E>>,
    read: bool
}

impl<T, E=()> Future<T, E>
    where T: Send + 'static,
          E: Send + 'static
{
    /// ```
    /// use tangle::{Future, Async};
    ///
    /// Future::new(|| if true { Async::Ok(123) } else { Async::Err("Foobar") });
    /// ```
    pub fn new<F>(f: F) -> Future<T, E>
        where F: FnOnce() -> Async<T, E> + Send + 'static
    {
        let (tx, rx) = channel();

        POOL.lock().unwrap().execute(move || { tx.send(f()); });

        Future::<T, E> {
            receiver: rx,
            read: false
        }
    }

    pub fn from_async_channel(receiver: Receiver<Async<T, E>>) -> Future<T, E> {
        Future::<T, E> {
            receiver: receiver,
            read: false
        }
    }

    /// Create a new future from the receiving end of a native channel.
    ///
    /// ```
    /// use tangle::{Future, Async};
    /// use std::thread;
    /// use std::sync::mpsc::channel;
    ///
    /// let (tx, rx) = channel();
    /// Future::<u32>::from_channel(rx).and_then(|v| {
    ///     assert_eq!(v, 1235);
    ///     Async::Ok(())
    /// });
    /// tx.send(1235);
    /// ```
    pub fn from_channel(receiver: Receiver<T>) -> Future<T, E> {
        let (tx, rx) = channel();

        POOL.lock().expect("error acquiring a lock.").execute(move || {
            match receiver.recv() {
                Ok(v) => { tx.send(Async::Ok(v)).expect("error sending on to the channel.") },
                Err(err) => { panic!("{:?}", err) }
            };
        });

        Future::<T, E> {
            receiver: rx,
            read: false
        }
    }

    /// ```
    /// use tangle::{Future, Async};
    ///
    /// let f: Future<usize> = Future::unit(1);
    ///
    /// let purchase: Future<String> = f.and_then(|num| Async::Ok(num.to_string()));
    ///
    /// match purchase.await() {
    ///     Async::Ok(val) => assert_eq!(val, "1".to_string()),
    ///     _ => {}
    /// }
    /// ```
    pub fn and_then<F, S>(self, f: F) -> Future<S, E>
        where F: FnOnce(T) -> Async<S, E> + Send + 'static,
              S: Send + 'static
    {
        let (tx, rx) = channel();

        POOL.lock().expect("error acquiring a lock.").execute(move || {
            match self.await() {
                Async::Ok(val) => {
                    tx.send(f(val));
                },
                Async::Err(err) => {
                    tx.send(Async::Err(err));
                },
                // We should never get to this point.
                _ => {}
            }
        });

        Future::<S, E> {
            receiver: rx,
            read: false
        }
    }

    /// ```
    /// use tangle::{Future, Async};
    ///
    /// let f: Future<usize> = Future::unit(1);
    ///
    /// let purchase: Future<String> = f.map(|num| num.to_string());
    ///
    /// match purchase.await() {
    ///     Async::Ok(val) => assert_eq!(val, "1".to_string()),
    ///     _ => {}
    /// }
    /// ```
    pub fn map<F, S>(self, f: F) -> Future<S, E>
        where F: FnOnce(T) -> S + Send + 'static,
              S: Send + 'static
    {
        let (tx, rx) = channel();

        POOL.lock().expect("error acquiring a lock.").execute(move || {
            match self.await() {
                Async::Ok(val) => {
                    tx.send(Async::Ok(f(val)));
                },
                Async::Err(err) => {
                    tx.send(Async::Err(err));
                },
                // We should never get to this point.
                _ => {}
            }
        });

        Future::<S, E> {
            receiver: rx,
            read: false
        }
    }

    pub fn await(self) -> Async<T, E> {
        let val = self.receiver.recv().expect("error trying to wait for channel.");

        match val {
            Async::Ok(val) => Async::Ok(val),
            Async::Err(err) => Async::Err(err),
            Continue(f) => f.await()
        }
    }

    /// Wrap a value into a `Future` that completes right away.
    ///
    /// ## Usage
    ///
    /// ```
    /// use tangle::Future;
    ///
    /// let _: Future<usize> = Future::unit(5);
    /// ```
    pub fn unit(val: T) -> Future<T, E> {
        let (tx, rx) = channel();

        tx.send(Async::Ok(val));

        Future::<T, E> {
            receiver: rx,
            read: false
        }
    }

    /// ```
    /// use tangle::Future;
    ///
    /// let _: Future<usize, &str> = Future::err("foobar");
    /// ```
    pub fn err(err: E) -> Future<T, E> {
        let (tx, rx) = channel();

        tx.send(Async::Err(err));

        Future::<T, E> {
            receiver: rx,
            read: false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use std::sync::mpsc::{channel, Receiver, Sender};

    #[test]
    fn async_macro() {
        fn foo() -> Async<u32, ()> {
            let v = Ok(123);
            let v = async!(v);
            assert_eq!(v, 123);
            Async::Ok(v)
        }

        foo();
    }

    #[test]
    fn async_macro_err() {
        fn foo() -> Async<u32, u32> {
            let v = Err(123u32);
            let v = async!(v);
            Async::Ok(0)
        }

        assert!(foo().is_err());
    }

    #[test]
    fn async_map() {
        let val: Async<u32, ()> = Async::Ok(123);
        match val.map(|x| x * 2) {
            Async::Ok(v) => assert_eq!(v, 246),
            _ => panic!("Unexpected error.")
        }
    }

    #[test]
    #[should_panic]
    fn async_map_fail() {
        let val: Async<u32, ()> = Async::Continue(future! {
            Async::Ok(123)
        });

        val.map(|x| x * 2);
    }

    #[test]
    fn async_map_err() {
        let val: Async<(), u32> = Async::Err(1);
        match val.map_err(|x| x + 5) {
            Async::Err(e) => assert_eq!(e, 6),
            _ => panic!("Unexpected error")
        }
    }

    #[test]
    fn from_chan1() {
        let (tx, rx) = channel();
        let f: Future<u32> = Future::from_channel(rx);

        // await
        let f = f.map(|n| {
            assert_eq!(n, 555);
            n + 5
        }).map(|n| {
            assert_eq!(n, 560);
            n
        });

        // Resolve the future through the sender half of the channel.
        tx.send(555);
    }

    #[test]
    fn to_future_macro() {
        future! {
            if true {
                Async::Ok(123)
            } else {
                Async::Err(5)
            }
        };
    }

    #[test]
    fn test_async() {
        let f: Future<usize> = future! { Async::Ok(5) };

        let next = f.and_then(|n| {
            thread::sleep(Duration::from_millis(50));

            Continue(Future::new(move || {
                thread::sleep(Duration::from_millis(100));
                Async::Ok(n * 100)
            }))
        });

        match next.await() {
            Async::Ok(n) => assert_eq!(n, 500),
            _ => panic!("Unexpected value")
        }
    }

    #[test]
    fn resolve_from_value() {
        let val: Future<usize> = Future::unit(5);

        match val.await() {
            Async::Ok(n) => assert_eq!(n, 5),
            _ => panic!("Unexpected value")
        }
    }

    #[test]
    fn map_promise() {
        let count: Future<usize> = Future::unit(5);

        let curr: Future<usize> = count.and_then(|n| {
            Continue(future! {
                Async::Ok(100)
            })
        });

        match curr.await() {
            Async::Ok(n) => assert_eq!(n, 100),
            _ => panic!("Unexpected value")
        }
    }

    // #[test]
    // fn promise() {
    //     let mut m = Promise::new();

    //     // Do some calculation...
    //     m.success(123);

    //     m.future().await()
    // }
}
