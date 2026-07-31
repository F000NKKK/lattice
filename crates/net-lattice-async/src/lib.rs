//! Runtime-agnostic async adapters for Net Lattice.
//!
//! [`stream`] bridges the synchronous, blocking
//! [`net_lattice_platform::EventReceiver`] onto a `futures::Stream`. It
//! deliberately creates one worker thread: `std::sync::mpsc::Receiver` has no
//! waker-registration mechanism, so a direct `Stream` implementation would
//! block an executor thread. No Tokio, async-std, or smol dependency is
//! imposed.

use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;

use futures::Stream;
use futures::channel::mpsc::{UnboundedReceiver, unbounded};
pub use net_lattice_core::{Error, Result};
use net_lattice_platform::EventReceiver;

/// A runtime-agnostic [`Stream`] forwarding events from a synchronous watcher.
pub struct EventStream<E> {
    receiver: UnboundedReceiver<Result<E>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

/// Bridges a synchronous event receiver to a waker-aware stream.
///
/// The returned stream owns the receiver. Dropping it requests worker shutdown
/// and joins the thread; shutdown latency is at most 50 ms, after which the
/// receiver is dropped and its backend subscription is cancelled.
pub fn stream<E>(receiver: EventReceiver<E>) -> EventStream<E>
where
    E: Send + 'static,
{
    let (sender, async_receiver) = unbounded();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let worker = thread::spawn(move || {
        while !worker_stop.load(Ordering::Acquire) {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(event)) => {
                    if sender.unbounded_send(Ok(event)).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = sender.unbounded_send(Err(error));
                    break;
                }
            }
        }
    });
    EventStream {
        receiver: async_receiver,
        stop,
        worker: Some(worker),
    }
}

impl<E> Stream for EventStream<E> {
    type Item = Result<E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

impl<E> Drop for EventStream<E> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Tokio-specific event stream, available with the `tokio` feature.
#[cfg(feature = "tokio")]
pub struct TokioEventStream<E> {
    receiver: tokio::sync::mpsc::UnboundedReceiver<Result<E>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

/// Bridges a watcher to Tokio's waker-aware channel.
#[cfg(feature = "tokio")]
pub fn tokio_stream<E>(receiver: EventReceiver<E>) -> TokioEventStream<E>
where
    E: Send + 'static,
{
    let (sender, async_receiver) = tokio::sync::mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let worker = thread::spawn(move || {
        while !worker_stop.load(Ordering::Acquire) {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(event)) => {
                    if sender.send(Ok(event)).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = sender.send(Err(error));
                    break;
                }
            }
        }
    });
    TokioEventStream {
        receiver: async_receiver,
        stop,
        worker: Some(worker),
    }
}

#[cfg(feature = "tokio")]
impl<E> Stream for TokioEventStream<E> {
    type Item = Result<E>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

#[cfg(feature = "tokio")]
impl<E> Drop for TokioEventStream<E> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn worker_forwards_events_to_the_stream() {
        let (sender, receiver) = EventReceiver::bounded();
        let mut events = stream(receiver);
        assert!(sender.send(7_u8, 0));
        assert!(matches!(
            futures::executor::block_on(events.next()),
            Some(Ok(7))
        ));
    }

    #[test]
    fn worker_forwards_receiver_errors() {
        let (sender, receiver) = EventReceiver::<u8>::bounded();
        let mut events = stream(receiver);
        assert!(sender.send_error(Error::InvalidState));
        assert!(matches!(
            futures::executor::block_on(events.next()),
            Some(Err(Error::InvalidState))
        ));
    }
}
