# deluge-maintain

A service that connects to one or more Deluge instances and applies retention policies on a schedule.

## Policy Engine

### Cron Scheduling

Each policy has a cron expression (`tokio-cron-scheduler`). All cron expressions are validated at config load time - a
malformed expression fails immediately, not silently at runtime.

At each scheduled tick for a given policy, the engine loops over all configured hosts.

### Per-Host Execution Flow

1. Fetch all torrents (only requesting needed fields) and free space via the service layer.
2. Check if any conditions are met (OR logic - any one condition triggers cleanup).
3. If no conditions are met, do nothing.
4. Filter torrents (all filters AND'd together - torrent must match every filter to be eligible).
5. If no torrents pass the filter, log a warning that conditions are met but nothing is eligible.
6. Sort filtered torrents by deletion priority (see below).
7. Simulate deletions greedily: walk the sorted list, accumulating `total_wanted` as freed space, until no conditions
   are met.
    - `available_space` condition: `simulated_free_space = initial_free_space + sum(total_wanted of planned deletions)`
    - `used_space` condition: `simulated_used_space = sum(total_wanted of remaining torrents)`
    - `total_count` condition: `simulated_count = remaining torrent count`
8. If conditions cannot be satisfied even after deleting all filtered torrents, log a warning.
9. If deletions are planned and `dry_run` is false, delete one torrent at a time, sleeping 1 second between deletions.

### Filters (AND logic)

All optional. A torrent must match every specified filter to be eligible for deletion.

| Filter                   | Type                    | Description                                         |
|--------------------------|-------------------------|-----------------------------------------------------|
| `age`                    | `humantime::Duration`   | Minimum time since torrent was added (`time_added`) |
| `ratio`                  | `f32`                   | Minimum seeding ratio                               |
| `completed`              | `bool` (default `true`) | Only consider completed torrents (`is_finished`)    |
| `min_total_seeds`        | `u32`                   | Minimum total seeds in swarm (`total_seeds`)        |
| `min_distributed_copies` | `f32`                   | Minimum distributed copies (`distributed_copies`)   |

### Conditions (OR logic)

All optional. If any condition is true, cleanup is triggered. Cleanup stops when all conditions are false.

| Condition         | Type                 | Description                                                   |
|-------------------|----------------------|---------------------------------------------------------------|
| `available_space` | `bytesize::ByteSize` | Free space at or below this threshold                         |
| `used_space`      | `bytesize::ByteSize` | Used space (sum of `total_wanted`) at or above this threshold |
| `total_count`     | `u32`                | Torrent count at or above this threshold                      |

### Sort Order (Deletion Priority)

Delete torrents that have the least impact on the swarm first:

1. `distributed_copies` **descending** - most copies = safest to remove
2. `age` **ascending** - oldest torrent first as tiebreaker (`now - time_added`)

### Dry-Run

When `dry_run` is true, the engine plans deletions and logs what would be deleted, but performs no RPC deletion calls.
The simulation step still runs to verify the plan is feasible.

### Error Handling

If a host is unreachable or an RPC call fails, skip that host, log a warning, and continue with other hosts. A single
host failure does not abort the run.

## Service Layer

RPC calls are abstracted behind a `DelugeService` trait using generics (not trait objects - no `#[async_trait]`, native
async traits with concrete generic types):

```rust
trait DelugeService {
    async fn get_torrents(&self) -> anyhow::Result<Vec<TorrentEntry>>;
    async fn get_free_space(&self) -> anyhow::Result<i64>;
    async fn remove_torrent(&self, hash: &str, remove_data: bool) -> anyhow::Result<()>;
}
```

Production wraps `DelugeClient`. Tests use a mock impl with fixture data.

Torrent field selection is minimized - only request the fields the engine uses: `name`, `hash`, `time_added`, `ratio`,
`is_finished`, `total_seeds`, `total_peers`, `distributed_copies`, `total_wanted`.

Deletions always use `remove_data = true` (the goal is freeing disk space).

## Testing

- **Unit tests**: filter matching, condition checking, sort order, deletion planning
  (simulation). All use plain `TorrentStatus` structs - no RPC.
- **Integration tests** (`tests/`): end-to-end engine pipeline with fixture data, verifying the full plan -> simulate ->
  verify cycle.
- **Not tested**: RPC client itself (deluge-rpc-client's responsibility), cron scheduling (tokio-cron-scheduler's
  responsibility), tracing output.

The engine's core logic (`plan_deletions`) is a pure method on the `Engine` struct that takes data in and returns a
deletion plan. The RPC calls are in a separate method. This is the testable boundary.

## Style Constraints

- Spread code across files, keep files small. Keep `impl` blocks with their struct definitions.
- No blank lines between `use` statements - single contiguous block, let the formatter sort groups.
- No `#[async_trait]` - use generics with native async traits instead of trait objects.
- Structs with impls and methods over detached free functions (OOP style).
- Lints copied from kagi-mcp (clippy pedantic subset, no `missing_docs`).
- Edition 2024, rust-version 1.85.
- `humantime` + `humantime-serde` for durations, `bytesize` with serde feature for data sizes.
- Tests use `when_<condition>_then_<action>_should_<expected>` naming, AAA structure with blank line separation.

## Logging

Use `tracing` for logging. Import the logging modules, e.g., `use tracing::info;` instead of using the absolute path,
e.g., `tracing::info!("Logging...");`.

Logging should always be in Sentence case (the first letter capitalized, proper names capitalized, using proper grammar,
etc.).

Logging Levels:

- `error`: For reporting errors that are not expected to occur during normal operation and typically require human
  intervention.
- `warn`: For reporting non-critical (recoverable) issues that may indicate a problem, but do not typically require
  human intervention.
- `info`: For reporting general information about the application's state or progress. Should be useful for an end-user
  to understand the application's behavior.
- `debug`: For detailed information that is primarily useful to developers.
- `trace`: For extremely detailed information required for low-level debugging.

## Commands

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features
cargo doc --workspace --no-deps
```
