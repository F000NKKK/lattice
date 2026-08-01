# net-lattice-model

Operating-system-independent observed and desired network models: interfaces,
addresses, routes, neighbors, DNS, events, and mutation plans.

Mutation plans are data only. Execution belongs to the `net-lattice` facade,
which uses the Stage 0.15 transaction executor and `ExecutionOptions`.

The published `net-lattice` facade connects these models to native backends.

## Example

```rust
use net_lattice_model::MutationPlan;

let plan = MutationPlan::default();
assert!(plan.is_empty());
```

Plans contain data only and never access the operating system.
