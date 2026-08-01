# net-lattice-async

Runtime-independent asynchronous event-stream adapter for Net Lattice. It
exposes a `futures::Stream` surface while platform backends provide native
watcher transports behind the facade's optional `async` feature.

Enable the `async` feature on the published `net-lattice` crate to use it.

## Example

```rust
use futures::StreamExt;

async fn consume<S>(mut stream: S)
where
    S: futures::Stream<Item = net_lattice_model::Event> + Unpin,
{
    while let Some(event) = stream.next().await {
        println!("{event:?}");
    }
}
```
