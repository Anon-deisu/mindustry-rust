# mdt-client-min Internal Boundaries

This note is a local placement guide for new `mdt-client-min` work.
It does not redefine release scope or claim full Java parity.

## Default Parity Baseline

- Unless a task explicitly names another target, parity comparison/default upstream baseline is `D:/MDT/mindustry-upstream-v157.4`.
- This baseline tracks upstream latest release `v157.4` (2026-04-22).

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

## Runtime Observability Contract: `controller_v2`

`controller_v2` is the preferred runtime observability contract for owned-unit controller state.
Treat it as a structured DTO exposed from business/session projections into runtime render/UI layers.
Do not introduce new controller meaning by extending opaque text labels first.

### Contract Shape

- `controller_v2` is the source-of-truth contract for runtime observability.
- It should carry structured fields with stable semantics that UI/render layers can format deterministically.
- Legacy controller detail text may still exist for compatibility, but it is a fallback/output bridge rather than the primary contract.

### Legacy Compatibility

- Keep legacy `detail`/string output only for compatibility with existing HUD, ASCII, window, or test expectations.
- New logic should read structured `controller_v2` fields first and derive text from them.
- Do not add new controller-only semantics exclusively inside legacy detail strings.

### Minimal Field Semantics

At minimum, `controller_v2` should preserve the smallest stable meaning set needed by runtime observability:

- controller kind/type identity
- controller value/entity/unit reference identity when present
- command/control mode linkage needed by runtime summaries
- queue/selection/target summary bits only when they are part of controller meaning rather than unrelated UI state

If a field is not required to preserve controller meaning across runtime summaries, do not force it into the base contract.

### Modification Rules

- Extend `controller_v2` only when the new field has stable cross-layer meaning and more than one consumer would otherwise need to re-parse text.
- Prefer additive changes; avoid renaming or redefining existing structured semantics without updating all downstream formatters/tests together.
- When adding a new structured field, keep legacy text output behavior compatible until all consumers are migrated.
- If text formatting changes are required, treat them as a formatter/output change layered on top of the structured contract, not as the contract itself.

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
