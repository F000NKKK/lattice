use std::any::Any;
use std::pin::Pin;
use std::sync::{
    Arc,
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
}

/// Tokio receiver that owns the native subscription which produces its events.
///
/// The subscription is dropped with the receiver, cancelling platform handles,
/// tasks, or reader threads before their callback state can be freed.
pub struct TokioEventReceiver<E> {
    receiver: tokio::sync::mpsc::Receiver<Result<E>>,
    subscription: Option<Box<dyn Any + Send>>,
}

impl<E> TokioEventReceiver<E> {
    /// Creates a bounded transport using the same capacity as `EventReceiver`.
    pub fn bounded() -> (TokioEventSender<E>, Self) {
        let (sender, receiver) = tokio::sync::mpsc::channel(EventReceiverCapacity::VALUE);
        let pending = Arc::new(AtomicBool::new(false));
        (
            TokioEventSender {
                sender,
                resync_pending: pending,
            },
            Self {
                receiver,
                subscription: None,
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
        Pin::new(&mut self.receiver).poll_recv(cx)
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
        self.sender.try_send(Err(error)).is_ok()
    }
}

/// Kept here instead of exposing a second public capacity constant.
struct EventReceiverCapacity;
impl EventReceiverCapacity {
    const VALUE: usize = 256;
}
