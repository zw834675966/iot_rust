# iot_gateway

Binary crate: Modbus TCP collector → JSON → local MQTT (embedded `rumqttd` + `rumqttc`). Simulator stands in for a real RS485/Modbus gateway; FUXA is the intended subscriber.

## Commands

```text
cargo run
cargo test --all-targets
cargo fmt --all
cargo fmt-check
cargo lint
```

`cargo lint` is `clippy --all-targets -- -D warnings`. Do not put `#![deny(warnings)]` in source.

Verify a change with **fmt-check → lint → test**, in that order.

## Stack

- Edition 2021, toolchain `stable` + `rustfmt` + `clippy` (`rust-toolchain.toml`)
- Runtime: `tokio`
- I/O: `tokio-modbus`, `rumqttd`, `rumqttc`
- Historian: `rusqlite` (bundled)
- Planned (do not delete as unused): `toml`, `arc-swap`
- Errors: `anyhow` + `?` (application crate)
- Logs: `tracing` / `tracing-subscriber`, never `println!` / `dbg!` on production paths

## Layout

- `src/main.rs` — wiring only
- `src/mqtt.rs` — broker config/start + client + publish
- `src/modbus_collector.rs` — TCP simulator + periodic collector
- `src/db.rs` — SQLite historian (`tag_history`)
- `src/lib.rs` — `pub mod db` for tests / reuse; keep it thin

Keep modules small. Do not invent a workspace, `lib.rs`, or extra crates until there is a second binary or a reusable library.

## Conventions

- rustfmt is law. Do not hand-format around it.
- No `unsafe`.
- No `.unwrap()` on I/O or protocol paths. `.expect("invariant: …")` only for broken invariants.
- Prefer `&str` / borrowing; clone only at task/thread boundaries (already true for MQTT publish).
- Tests live in `#[cfg(test)]` next to the code they cover. `unwrap` in tests is allowed (`clippy.toml`).
- Do not add a crate if std / an existing dep already does the job.

## Boundaries

- Broker and simulator bind **127.0.0.1** unless the user asks otherwise.
- Never commit secrets, `.env*`, or `*.db`.
- Do not rewrite working modules to add traits, builders, or error enums “for later”.
- SQLite history and FUXA HMI verification are pending; leave the reserved deps in `Cargo.toml`.
