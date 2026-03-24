# Agent Lifecycle Gap (2026-03-24)

Date: 2026-03-24
Scope: Java `NetClient` / `Net` lifecycle vs Rust `mdt-client-min`
Focus: `worldDataBegin`, `clientLoaded`, `finishConnecting`, `connectConfirm`, timeout/watchdog, reconnect/reset

## Java Key Semantics

- `worldDataBegin` is a lifecycle transition, not only cache clearing.
  - Java clears world/entity-side runtime ownership, resets logic, sets `connecting=true`, flips `clientLoaded=false`, and re-enters loading UI.
- `clientLoaded` is a network ingress gate.
  - high-priority packets bypass the gate
  - normal packets queue while `clientLoaded=false`
  - replay happens when the gate opens
- `finishConnecting()` is a single serial commit point.
  - it transitions into playing state
  - clears connecting/join UI state
  - flips `clientLoaded=true`
  - posts `connectConfirm`
  - arms the ready-state snapshot watchdog
- timeout semantics split into two phases.
  - loading/connect timeout before ready
  - snapshot-stall timeout after finish-connecting

## Rust Current Boundary

- Rust already has a loading-time gate around `worldDataBegin` / world-stream load.
  - normal packets defer
  - low-priority packets drop
  - stream packets still flow
- `mark_client_loaded()` already replays deferred packets fail-closed and auto-queues `connectConfirm`.
- current Rust still treats `connectConfirm queued` and `finish-connecting complete` too closely.
  - actual transport flush still happens later in the transport loops
- split-driver transport still exposes contract gaps.
  - especially around TCP-only actions when a UDP-owned advance path is active
- `worldDataBegin` reset is broad, but it still does not model Java's full finish-connecting / UI / reconnect atomicity.

## Best Next Serial Slices

1. Add an explicit `finishConnecting` commit helper/phase.
   - collapse: world ready, deferred replay exhausted, `client_loaded=true`, `connectConfirm` queued/flushed state, snapshot watchdog arming
   - write scope:
     - `rust/mdt-client-min/src/client_session.rs`
     - `rust/mdt-client-min/src/bootstrap_flow.rs`
     - `rust/mdt-client-min/src/session_state.rs`

2. Split `connectConfirm queued` from `connectConfirm flushed`.
   - do not keep treating queued-as-complete
   - make the transport contract explicit for split-driver paths
   - write scope:
     - `rust/mdt-client-min/src/client_session.rs`
     - `rust/mdt-client-min/src/arcnet_loop.rs`
     - `rust/mdt-client-min/src/udp_loop.rs`

3. Unify `worldDataBegin` / reconnect / redirect reset ownership.
   - give one serial owner to:
     - world reload begin
     - quiet reconnect reset
     - redirect transition
     - `serverRestarting` kick transition
   - write scope:
     - `rust/mdt-client-min/src/client_session.rs`
     - `rust/mdt-client-min/src/session_state.rs`

4. Tighten timeout/watchdog phase switching.
   - keep separate loading-time timeout and ready-state snapshot stall timeout
   - switch the anchor only through the explicit finish-connecting phase
   - write scope:
     - `rust/mdt-client-min/src/client_session.rs`
     - `rust/mdt-client-min/src/session_state.rs`

5. Add lifecycle proof tests before widening behavior.
   - cover:
     - `finishConnecting`
     - `clientLoaded=1`
     - `connectConfirmPosted/queued/flushed`
     - pre-load replay
     - second world load / reconnect
   - write scope:
     - `rust/mdt-client-min/src/client_session.rs`
     - `rust/mdt-client-min/src/bootstrap_flow.rs`

## Conclusion

The remaining lifecycle gap is no longer "missing a loading gate". The gap is serial atomicity: Java-style `finishConnecting` still needs a stronger single-owner transition that closes transport, watchdog, replay, and reconnect/reset semantics together.
