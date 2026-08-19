//! The minimal future driver.
//!
//! A specification crate should not choose anyone's async runtime. The traits here use `async fn`
//! because the contract has to be implementable by a backend that talks to a network; a backend that
//! talks to a `BTreeMap` implements them with futures that complete on the first poll, and this
//! drives one to completion without a dependency.
//!
//! It is not a runtime. It busy-polls, which is exactly right for a future that never yields and
//! exactly wrong for one that does — so it belongs in tests and in synchronous backends, nowhere else.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

/// Drives `future` to completion by polling it.
///
/// # Panics
///
/// Panics if the future is still pending after a bounded number of polls, which for a synchronous
/// backend means it is waiting on something no waker will ever signal — a deadlock, reported rather
/// than hung.
pub fn block_on<F: Future>(future: F) -> F::Output {
    // `Waker::noop` is exactly right here and needs no `unsafe`: nothing will ever wake this future,
    // because a synchronous backend's futures are ready on the first poll.
    let mut context = Context::from_waker(Waker::noop());
    let mut future = pin!(future);

    for _ in 0..1_000_000 {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::hint::spin_loop();
    }
    panic!(
        "the future did not complete: a synchronous backend must not await anything that needs a \
         runtime to make progress"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_a_future_that_completes_immediately() {
        // A future that never yields is exactly the shape a synchronous backend produces.
        #[allow(clippy::unused_async)]
        async fn answer() -> u32 {
            41 + 1
        }
        assert_eq!(block_on(answer()), 42);
    }

    #[test]
    fn drives_a_chain_of_awaits() {
        #[allow(clippy::unused_async)]
        async fn inner(value: u32) -> u32 {
            value * 2
        }
        async fn outer() -> u32 {
            inner(inner(3).await).await
        }
        assert_eq!(block_on(outer()), 12);
    }
}
