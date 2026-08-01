# net-lattice-async

Runtime-independent asynchronous event-stream adapter for Net Lattice.

## What it provides

- `EventStream<E>`, a `futures::Stream<Item = Result<E>>` surface;
- adaptation of blocking `EventReceiver` transports using one worker thread;
- direct wrapping of backend-native Tokio receivers;
- deterministic shutdown when the stream is dropped.

Applications normally enable the `async` feature on `net-lattice` and obtain
the stream from its facade. Depend on this crate directly only when adapting a
custom backend transport.

## Usage

```rust
use futures::StreamExt;
use net_lattice_async::{EventStream, Result};

async fn next_event<E>(stream: &mut EventStream<E>) -> Option<Result<E>>
where
    E: Unpin,
{
    stream.next().await
}
```

## Runtime behavior

The adapter does not select an async runtime. A blocking receiver uses a
dedicated worker because `std::sync::mpsc::Receiver` cannot register a waker;
a native Tokio receiver is exposed without that worker.
