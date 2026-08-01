# net-lattice-async

Runtime-independent asynchronous event-stream adapter for Net Lattice. It
exposes a `futures::Stream` surface while platform backends provide native
watcher transports behind the facade's optional `async` feature.

See the [workspace README](../../README.md) for usage examples.

