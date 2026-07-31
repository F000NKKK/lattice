//! Runtime-agnostic async adapters for Net Lattice.
//!
//! [`from_receiver`] bridges the synchronous, blocking
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
use net_lattice_platform::{EventReceiver, TokioEventReceiver};

/// A runtime-agnostic asynchronous stream of network change events.
///
/// Depending on the connected backend, events may be delivered through a
/// native asynchronous watcher or adapted from a synchronous
/// [`EventReceiver`]. Applications normally obtain this stream through
/// `Lattice::watch_async` with Net Lattice's `async` feature.
///
/// Implements [`Stream`].
pub struct EventStream<E> {
    receiver: EventStreamReceiver<E>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
enum EventStreamReceiver<E> {
    Futures(UnboundedReceiver<Result<E>>),
    Tokio(TokioEventReceiver<E>),
}

/// Bridges a synchronous event receiver to a waker-aware stream.
///
/// The returned stream owns the receiver. Dropping it requests worker shutdown
/// and joins the thread; shutdown latency is at most 50 ms, after which the
/// receiver is dropped and its backend subscription is cancelled.
pub fn from_receiver<E>(receiver: EventReceiver<E>) -> EventStream<E>
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
        receiver: EventStreamReceiver::Futures(async_receiver),
        stop,
        worker: Some(worker),
    }
}

/// Wraps a backend-native Tokio event receiver in the same public stream.
pub fn from_tokio_receiver<E>(receiver: TokioEventReceiver<E>) -> EventStream<E> {
    EventStream {
        receiver: EventStreamReceiver::Tokio(receiver),
        stop: Arc::new(AtomicBool::new(false)),
        worker: None,
    }
}

impl<E> Stream for EventStream<E> {
    type Item = Result<E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match &mut self.receiver {
            EventStreamReceiver::Futures(receiver) => Pin::new(receiver).poll_next(cx),
            EventStreamReceiver::Tokio(receiver) => Pin::new(receiver).poll_recv(cx),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn worker_forwards_events_to_the_stream() {
        let (sender, receiver) = EventReceiver::bounded();
        let mut events = from_receiver(receiver);
        assert!(sender.send(7_u8, 0));
        assert!(matches!(
            futures::executor::block_on(events.next()),
            Some(Ok(7))
        ));
    }

    #[test]
    fn worker_forwards_receiver_errors() {
        let (sender, receiver) = EventReceiver::<u8>::bounded();
        let mut events = from_receiver(receiver);
        assert!(sender.send_error(Error::InvalidState));
        assert!(matches!(
            futures::executor::block_on(events.next()),
            Some(Err(Error::InvalidState))
        ));
    }

    #[test]
    fn native_tokio_receiver_uses_the_same_stream_surface() {
        let (sender, receiver) = TokioEventReceiver::bounded();
        let mut events = from_tokio_receiver(receiver);
        assert!(sender.send(7_u8, || 0));
        drop(sender);
        assert!(matches!(
            futures::executor::block_on(events.next()),
            Some(Ok(7))
        ));
        assert!(futures::executor::block_on(events.next()).is_none());
    }

    #[test]
    fn dropping_a_sync_stream_joins_its_adapter_worker() {
        let (_sender, receiver) = EventReceiver::<u8>::bounded();
        drop(from_receiver(receiver));
    }
}
