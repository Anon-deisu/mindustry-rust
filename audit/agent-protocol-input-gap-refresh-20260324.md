# Agent Protocol Input Gap Refresh (2026-03-24)

Purpose: keep a short, current list of Java-vs-Rust M6/M8 gaps that remain high-signal after the latest typed-registry, custom-channel, and low-risk input/runtime slices.

## High

- M8 `BuilderComp` execution semantics remain the largest gameplay/input gap.
  - Java reference:
    - `core/src/mindustry/entities/comp/BuilderComp.java`
  - Rust boundary:
    - `rust/mdt-client-min/src/client_session.rs`
    - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
    - `rust/mdt-input/**`
  - Rust now has queue projection, local reconciliation, and packet send paths, but it still does not match Java validation, range/resource checks, queue reorder, `beginPlace` / `beginBreak` lifecycle, and sustained build progression.

- M8 real-time input capture is still much narrower than Java desktop/mobile runtime input.
  - Java reference: `core/src/mindustry/core/NetClient.java` `sync()`
  - Rust boundary:
    - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
    - `rust/mdt-input/src/mapper.rs`
    - `rust/mdt-input/src/live_intent.rs`
  - Rust remains centered on CLI/runtime snapshots and simplified mapper semantics rather than real desktop/mobile input stacks.

- M6 `effect(..., data)` still stops well short of Java semantics.
  - Java reference:
    - `core/src/mindustry/io/TypeIO.java`
    - `core/src/mindustry/entities/Effect.java`
  - Rust boundary:
    - `rust/mdt-client-min/src/effect_runtime.rs`
    - `rust/mdt-client-min/src/client_session.rs`
    - `rust/mdt-client-min/src/render_runtime.rs`
  - Rust now has a narrow contract table plus executor overlays, but effect-id coverage and contract/business depth remain limited compared with Java `TypeIO.readObject` consumers.

## Medium

- M6 custom/logic remote now has typed glue, but business integration is still thin.
  - Java reference: `core/src/mindustry/core/NetClient.java`
  - Rust boundary:
    - `rust/mdt-client-min/src/typed_remote_dispatch.rs`
    - `rust/mdt-client-min/src/client_session.rs`
    - `rust/mdt-client-min/src/bin/mdt-client-min-online.rs`
  - Current Rust path is strong on registry/dispatch/watch/print coverage, but still partial on Java-equivalent business consumption.

## Notes

- Do not re-open already landed work such as typed registry glue, custom/logic watch flags, narrow landed effect contracts, or `tileConfig` FIFO reconciliation.
- Current high-value follow-up remains:
  - deeper `BuilderComp` semantics
  - broader live input capture
  - additional effect contract/executor families
  - custom/logic packet business adoption beyond observability
