use std::any::Any;
use std::pin::Pin;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::task::{Context, Poll};

use net_lattice_core::Result;

/// Tokio-backed half of a bounded event channel.
///
/// This is an implementation detail of the feature-gated async provider
/// contract. Backends use [`send`](Self::send) exactly as their synchronous
/// counterparts use `EventSender`: when the consumer falls behind, one
/// resynchronization event is scheduled instead of allowing unbounded memory
/// growth.
pub struct TokioEventSender<E> {
    sender: tokio::sync::mpsc::Sender<Result<E>>,
    resync_pending: Arc<AtomicBool>,
    terminal_error: Arc<Mutex<Option<net_lattice_core::Error>>>,
}

/// Tokio receiver that owns the native subscription which produces its events.
///
/// The subscription is dropped with the receiver, cancelling platform handles,
/// tasks, or reader threads before their callback state can be freed.
pub struct TokioEventReceiver<E> {
    receiver: tokio::sync::mpsc::Receiver<Result<E>>,
    subscription: Option<Box<dyn Any + Send>>,
    terminal_error: Arc<Mutex<Option<net_lattice_core::Error>>>,
}

impl<E> TokioEventReceiver<E> {
    /// Creates a bounded transport using the same capacity as `EventReceiver`.
    pub fn bounded() -> (TokioEventSender<E>, Self) {
        let (sender, receiver) = tokio::sync::mpsc::channel(EventReceiverCapacity::VALUE);
        let pending = Arc::new(AtomicBool::new(false));
        let terminal_error = Arc::new(Mutex::new(None));
        (
            TokioEventSender {
                sender,
                resync_pending: pending,
                terminal_error: Arc::clone(&terminal_error),
            },
            Self {
                receiver,
                subscription: None,
                terminal_error,
            },
        )
    }

    /// Attaches the resource which owns the OS subscription.
    pub fn with_subscription<S>(mut self, subscription: S) -> Self
    where
        S: Any + Send + 'static,
    {
        self.subscription = Some(Box::new(subscription));
        self
    }

    /// Polls the next event without tying the platform crate to a futures API.
    pub fn poll_recv(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<E>>> {
        match Pin::new(&mut self.receiver).poll_recv(cx) {
            Poll::Ready(None) => Poll::Ready(
                self.terminal_error
                    .lock()
                    .expect("Tokio event sender poisoned")
                    .take()
                    .map(Err),
            ),
            other => other,
        }
    }
}

impl<E> TokioEventSender<E> {
    /// Attempts to enqueue an event without ever blocking an OS callback.
    pub fn send(&self, event: E, resync: impl FnOnce() -> E) -> bool {
        if self.resync_pending.swap(false, Ordering::AcqRel) {
            match self.sender.try_send(Ok(resync())) {
                Ok(()) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    self.resync_pending.store(true, Ordering::Release);
                    return true;
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => return false,
            }
        }
        match self.sender.try_send(Ok(event)) {
            Ok(()) => true,
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                self.resync_pending.store(true, Ordering::Release);
                true
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    /// Reports a terminal backend error if the consumer is still connected.
    pub fn send_error(&self, error: net_lattice_core::Error) -> bool {
        if self.sender.is_closed() {
            return false;
        }
        *self
            .terminal_error
            .lock()
            .expect("Tokio event sender poisoned") = Some(error);
        true
    }
}

/// Kept here instead of exposing a second public capacity constant.
struct EventReceiverCapacity;
impl EventReceiverCapacity {
    const VALUE: usize = 256;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DropGuard(Arc<AtomicUsize>);

    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn poll<E>(receiver: &mut TokioEventReceiver<E>) -> Poll<Option<Result<E>>> {
        let waker = std::task::Waker::noop();
        let mut context = Context::from_waker(waker);
        Pin::new(receiver).poll_recv(&mut context)
    }

    #[test]
    fn overflow_delivers_resync_before_a_later_event() {
        let (sender, mut receiver) = TokioEventReceiver::bounded();
        for event in 0..EventReceiverCapacity::VALUE {
            assert!(sender.send(event, || 999));
        }
        assert!(sender.send(256, || 999));
        for event in 0..EventReceiverCapacity::VALUE {
            let _ = event;
            assert!(poll(&mut receiver).is_ready());
        }
        assert!(sender.send(257, || 999));
        assert!(poll(&mut receiver).is_ready());
    }

    #[test]
    fn terminal_error_survives_a_full_queue() {
        let (sender, mut receiver) = TokioEventReceiver::bounded();
        for event in 0..EventReceiverCapacity::VALUE {
            assert!(sender.send(event, || 999));
        }
        assert!(sender.send_error(net_lattice_core::Error::InvalidState));
        drop(sender);
        for _ in 0..EventReceiverCapacity::VALUE {
            assert!(poll(&mut receiver).is_ready());
        }
        assert!(poll(&mut receiver).is_ready());
    }

    #[test]
    fn subscription_is_dropped_and_closed_consumer_is_reported() {
        let drops = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = TokioEventReceiver::<u32>::bounded();
        drop(receiver.with_subscription(DropGuard(Arc::clone(&drops))));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(!sender.send(1, || 0));
        assert!(!sender.send_error(net_lattice_core::Error::InvalidState));
    }

    #[test]
    fn closed_consumer_is_reported_while_a_resync_is_pending() {
        let (sender, receiver) = TokioEventReceiver::bounded();
        for event in 0..EventReceiverCapacity::VALUE {
            assert!(sender.send(event, || 999));
        }
        assert!(sender.send(256, || 999));
        drop(receiver);
        assert!(!sender.send(257, || 999));
    }

    #[test]
    #[should_panic(expected = "Tokio event sender poisoned")]
    fn poisoned_tokio_terminal_error_mutex_is_reported() {
        let (sender, _receiver) = TokioEventReceiver::<u32>::bounded();
        let terminal_error = Arc::clone(&sender.terminal_error);
        let _ = std::thread::spawn(move || {
            let _guard = terminal_error.lock().unwrap();
            panic!("poison Tokio terminal error");
        })
        .join();
        let _ = sender.send_error(net_lattice_core::Error::InvalidState);
    }

    #[test]
    #[should_panic(expected = "Tokio event sender poisoned")]
    fn poisoned_tokio_receiver_mutex_is_reported() {
        let (sender, mut receiver) = TokioEventReceiver::<u32>::bounded();
        let terminal_error = Arc::clone(&receiver.terminal_error);
        let _ = std::thread::spawn(move || {
            let _guard = terminal_error.lock().unwrap();
            panic!("poison Tokio receiver");
        })
        .join();
        drop(sender);
        let _ = poll(&mut receiver);
    }

    #[test]
    fn pending_resync_stays_pending_while_the_tokio_channel_is_full() {
        let (sender, _receiver) = TokioEventReceiver::bounded();
        for event in 0..EventReceiverCapacity::VALUE {
            assert!(sender.send(event, || 999));
        }
        assert!(sender.send(256, || 999));
        assert!(sender.send(257, || 999));
    }

    #[test]
    fn poll_recv_reports_pending_while_connected_and_empty() {
        let (_sender, mut receiver) = TokioEventReceiver::<u32>::bounded();
        assert!(matches!(poll(&mut receiver), Poll::Pending));
    }

    fn exercise_sender_paths<E: Copy + Send + 'static>(event: E, resync: E) {
        let (sender, mut receiver) = TokioEventReceiver::bounded();
        for _ in 0..EventReceiverCapacity::VALUE {
            assert!(sender.send(event, || resync));
        }
        assert!(sender.send(event, || resync));
        assert!(sender.send(event, || resync));
        for _ in 0..EventReceiverCapacity::VALUE {
            assert!(poll(&mut receiver).is_ready());
        }
        assert!(sender.send(event, || resync));
        assert!(poll(&mut receiver).is_ready());
        drop(receiver);
        assert!(!sender.send(event, || resync));
        assert!(!sender.send_error(net_lattice_core::Error::InvalidState));
    }

    #[test]
    fn sender_paths_are_exercised_for_common_event_types() {
        exercise_sender_paths::<u32>(1, 99);
        exercise_sender_paths::<i32>(1, 99);
    }
}
