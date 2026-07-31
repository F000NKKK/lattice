use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use net_lattice_core::{Error, Result};

/// A bounded synchronous receiver of network change events.
///
/// [`Self::recv`] blocks, while [`Self::try_recv`] and
/// [`Self::recv_timeout`] do not wait indefinitely. Dropping the receiver
/// cancels its native watcher subscription. If its producer shuts down,
/// receive methods return [`Error::Disconnected`]. When a slow consumer
/// fills the bounded queue, multiple dropped events are coalesced into one
/// resynchronization event delivered before a later ordinary event.
///
/// `EventReceiver<E>` is `Send` when `E` is `Send`; it is not cloneable, so a
/// watcher has one consuming receiver.
///
/// Mirrors [`std::sync::mpsc::Receiver`] deliberately — a bare channel
/// receiver, not a `futures_core::Stream`, so that neither
/// `net-lattice-platform` nor `net-lattice-core` ever depend on an async
/// runtime (see ARCHITECTURE.md's Async Model: `EventProvider` is
/// inherently push-based on every platform, but that decision must not
/// force Tokio, async-std, smol, or any executor onto every consumer of
/// this crate). A consumer already committed to async can wrap this in
/// `spawn_blocking`, or a separate crate can offer a `Stream` adapter on
/// top without this crate ever knowing async exists.
///
/// Also implements [`Iterator`], for `for event in receiver { ... }` —
/// iteration ends (`None`) exactly when the channel disconnects, the same
/// way `mpsc::Receiver`'s `Iterator` impl does.
pub const DEFAULT_EVENT_QUEUE_CAPACITY: usize = 256;

/// Non-blocking backend producer for a bounded event receiver.
#[derive(Clone)]
pub struct EventSender<E> {
    sender: mpsc::SyncSender<Result<E>>,
    pending_resync: Arc<Mutex<Option<E>>>,
}

impl<E> EventSender<E> {
    /// Never blocks a native callback. On overflow, records one resync event
    /// that is delivered before a later ordinary event.
    pub fn send(&self, event: E, resync: E) -> bool {
        let mut pending = self.pending_resync.lock().expect("event sender poisoned");
        if let Some(resync) = pending.take() {
            match self.sender.try_send(Ok(resync)) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(Ok(resync))) => {
                    *pending = Some(resync);
                    return true;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => return false,
                Err(mpsc::TrySendError::Full(Err(_))) => unreachable!(),
            }
        }
        match self.sender.try_send(Ok(event)) {
            Ok(()) => true,
            Err(mpsc::TrySendError::Full(Ok(_))) => {
                *pending = Some(resync);
                true
            }
            Err(mpsc::TrySendError::Disconnected(_)) => false,
            Err(mpsc::TrySendError::Full(Err(_))) => unreachable!(),
        }
    }
    pub fn send_error(&self, error: Error) -> bool {
        self.sender.send(Err(error)).is_ok()
    }
}

pub struct EventReceiver<E> {
    receiver: mpsc::Receiver<Result<E>>,
    // Owns backend-specific cancellation state (for example, a Windows IP
    // Helper registration or a route-socket reader). It is intentionally
    // opaque: consumers only receive events, while dropping the receiver
    // reliably tears down the native subscription.
    _subscription: Option<Box<dyn Send>>,
}

impl<E> EventReceiver<E> {
    pub fn bounded() -> (EventSender<E>, Self) {
        Self::bounded_with_capacity(DEFAULT_EVENT_QUEUE_CAPACITY)
    }
    pub fn bounded_with_capacity(capacity: usize) -> (EventSender<E>, Self) {
        assert!(capacity > 0, "event queue capacity must be non-zero");
        let (sender, receiver) = mpsc::sync_channel(capacity);
        (
            EventSender {
                sender,
                pending_resync: Arc::new(Mutex::new(None)),
            },
            Self {
                receiver,
                _subscription: None,
            },
        )
    }
    /// Wraps a channel receiver a backend's background watcher thread/task
    /// sends events into. The `Sender` half is not exposed here — only the
    /// backend that spawned the watcher should be able to produce events.
    pub fn new(receiver: mpsc::Receiver<Result<E>>) -> Self {
        Self {
            receiver,
            _subscription: None,
        }
    }

    /// Associates a backend-owned cancellation guard with this receiver.
    /// Backends use this after registering a native watcher; dropping the
    /// receiver drops the guard and therefore stops the native subscription.
    pub fn from_receiver_with_subscription<S>(
        receiver: mpsc::Receiver<Result<E>>,
        subscription: S,
    ) -> Self
    where
        S: Send + 'static,
    {
        Self {
            receiver,
            _subscription: Some(Box::new(subscription)),
        }
    }
    pub fn with_subscription<S>(mut self, subscription: S) -> Self
    where
        S: Send + 'static,
    {
        self._subscription = Some(Box::new(subscription));
        self
    }

    /// Blocks until an event arrives. Returns `Err(Error::Disconnected)`
    /// once the backend's watcher has shut down and no further event will
    /// ever arrive.
    pub fn recv(&self) -> Result<E> {
        self.receiver.recv().map_err(|_| Error::Disconnected)?
    }

    /// Returns immediately: `Ok(Some(event))` if one is already queued,
    /// `Ok(None)` if none is available right now (not an error — there is
    /// simply nothing to report yet), or `Err(Error::Disconnected)` if the
    /// watcher has shut down.
    pub fn try_recv(&self) -> Result<Option<E>> {
        match self.receiver.try_recv() {
            Ok(Ok(event)) => Ok(Some(event)),
            Ok(Err(error)) => Err(error),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(Error::Disconnected),
        }
    }

    /// Blocks for at most `timeout`: `Ok(Some(event))` if one arrives in
    /// time, `Ok(None)` on timeout (again, not an error), or
    /// `Err(Error::Disconnected)` if the watcher has shut down.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<E>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(Ok(event)) => Ok(Some(event)),
            Ok(Err(error)) => Err(error),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Error::Disconnected),
        }
    }
}

impl<E> Iterator for EventReceiver<E> {
    type Item = E;

    fn next(&mut self) -> Option<E> {
        self.recv().ok()
    }
}

/// Subscribes to filtered change notifications for the domains and objects a
/// backend supports.
///
/// Generic over an associated `Event` type rather than naming
/// `net_lattice_model::event::Event` directly — `net-lattice-platform` does
/// not depend on `net-lattice-model` (see ARCHITECTURE.md). The facade
/// crate (`net-lattice`) is what constrains `Event` to the concrete model
/// type.
///
/// Unlike the other provider traits, this one is inherently push-based:
/// `watch` starts a backend-owned background watcher (a Netlink multicast
/// subscription, a BSD routing-socket reader, a Windows
/// `NotifyRouteChange2`-style callback, ...) and returns an
/// [`EventReceiver`] fed by it. The watcher runs for as long as the
/// returned `EventReceiver` (and whatever the backend keeps alive to feed
/// it) is alive.
pub trait EventProvider {
    type Event;
    type EventFilter;

    fn watch(&self) -> Result<EventReceiver<Self::Event>>;
    fn watch_filtered(&self, filter: Self::EventFilter) -> Result<EventReceiver<Self::Event>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn recv_returns_disconnected_once_the_sender_is_dropped() {
        let (sender, receiver) = EventReceiver::<u32>::bounded();
        drop(sender);
        assert!(matches!(receiver.recv(), Err(Error::Disconnected)));
    }

    #[test]
    fn try_recv_returns_none_when_empty_but_still_connected() {
        let (_sender, receiver) = EventReceiver::<u32>::bounded();
        assert!(matches!(receiver.try_recv(), Ok(None)));
    }

    #[test]
    fn iterator_ends_when_the_sender_is_dropped() {
        let (sender, receiver) = EventReceiver::<u32>::bounded();
        thread::spawn(move || {
            assert!(sender.send(1, 0));
            assert!(sender.send(2, 0));
        });
        let received: Vec<u32> = receiver.collect();
        assert_eq!(received, vec![1, 2]);
    }

    #[test]
    fn recv_timeout_returns_none_on_timeout_without_disconnecting() {
        let (sender, receiver) = EventReceiver::<u32>::bounded();
        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(10)),
            Ok(None)
        ));
        assert!(sender.send(7, 0));
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            Some(7)
        );
    }

    #[test]
    fn overflow_delivers_resync_before_a_later_event() {
        let (sender, receiver) = EventReceiver::bounded_with_capacity(1);
        assert!(sender.send(1, 99));
        assert!(sender.send(2, 99));
        assert_eq!(receiver.recv().unwrap(), 1);
        assert!(sender.send(3, 99));
        assert_eq!(receiver.recv().unwrap(), 99);
    }

    #[test]
    fn background_error_is_returned() {
        let (sender, receiver) = EventReceiver::<u32>::bounded();
        assert!(sender.send_error(Error::InvalidState));
        assert!(matches!(receiver.recv(), Err(Error::InvalidState)));
    }
}
