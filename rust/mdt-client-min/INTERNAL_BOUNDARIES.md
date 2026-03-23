# mdt-client-min Internal Boundaries

This note is a local placement guide for new `mdt-client-min` work.
It does not redefine release scope or claim full Java parity.

## Current Role Map

`client_session.rs`

- integration hub for manifest-bound packet ids, inbound dispatch, outbound queueing, and packet-level business apply
- acceptable for narrow packet-family wiring and thin coordination across modules
- not the right place for new long-lived state models when they can live in `session_state.rs`

`session_state.rs`

- authoritative session-facing state and lightweight business projections
- preferred home for new persistent mirrors such as configured-block, payload-lifecycle, resource-delta, or rules/objectives state
- keep mutation helpers close to the projection they update

`rules_objectives_semantics.rs`

- pure JSON-to-projection logic for rules/objectives
- preferred home for deterministic semantic parsing that should stay separate from packet transport concerns

`snapshot_ingest.rs`

- snapshot envelope/body parsing and authority/business projection folding
- avoid mixing unrelated remote-control or configured-block logic into this file

`render_runtime.rs`

- runtime HUD/status text and scene-facing projection summaries
- preferred home for compact observability labels instead of formatting strings inside `client_session.rs`

`event_summary.rs`

- human-readable summaries for packet events
- use when a new packet/projection needs print/watch output but not scene/HUD state

`arcnet_loop.rs`, `udp_loop.rs`, `net_loop.rs`, `bootstrap_flow.rs`, `connect_packet.rs`

- transport/bootstrap/liveness path
- any change touching reconnect, `finishConnecting`, `clientLoaded`, deferred replay, or `worldDataBegin` is high-conflict work

## Placement Rules

- New persistent business mirrors go in `session_state.rs` first.
- Pure decode or semantic normalization helpers should not be added to runtime/UI files.
- New HUD/status labels belong in `render_runtime.rs`.
- New print/watch summaries belong in `event_summary.rs`.
- If a change only needs packet dispatch plus projection updates, keep it out of snapshot/bootstrap files.

## High-Conflict Areas

Treat these as serial lanes unless there is a strong reason not to:

- `client_session.rs` logic around `finishConnecting`, `clientLoaded`, deferred packet replay, and `worldDataBegin`
- snapshot authority/business apply flow spanning `snapshot_ingest.rs` and `session_state.rs`
- transport reconnect state in `arcnet_loop.rs`

## Low-Conflict Extension Lanes

These are usually safe to extend without rewriting the core state machine:

- configured-block business projection
- rules/objectives semantic projection
- resource/payload lifecycle projection
- custom packet runtime/watch layers
- HUD/status observability

## Immediate Guidance

When adding a new parity slice:

1. Put durable state in `session_state.rs`.
2. Keep packet-family dispatch in `client_session.rs` thin.
3. Move pure normalization into a dedicated helper/module when the logic can stand alone.
4. Only add runtime text once the state shape is stable.
