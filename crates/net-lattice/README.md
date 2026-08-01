# net-lattice

The public Net Lattice facade for cross-platform network inspection and
configuration. It selects the native backend for Linux, Windows, or macOS and
exposes one strongly typed `Lattice` API.

The facade covers routes, interfaces, addresses, DNS, neighbors, monitoring,
and Stage 0.15 ordered mutation-plan execution through `ExecutionOptions`.
Enable the optional `async` feature for the runtime-independent event stream.

See the [main README](../../README.md),
[Russian README](../../README.ru.md), and
[architecture](../../ARCHITECTURE.md) for complete documentation.

