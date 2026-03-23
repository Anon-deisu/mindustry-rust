# CI Gate Plan (Track P3)

Reference-only note for `mindustry-rust` handoff:

- this document records the current source-monorepo release-gate design and governance
- syncing this file into the target repo does not by itself mean `.github/workflows/rust-release-gate.yml` is already present there
- treat workflow wiring, branch protection, and required-check setup in the target repo as separate follow-up work unless the workflow file is explicitly synced and enabled

## Source of Truth

- Workflow file: `.github/workflows/rust-release-gate.yml`

## Source Monorepo CI Snapshot

- Existing workflow set in the source monorepo:
  - `.github/workflows/pr.yml`
  - `.github/workflows/push.yml`
  - `.github/workflows/deployment.yml`
  - `.github/workflows/gradle-wrapper-validation.yml`
  - `.github/workflows/rust-release-gate.yml`
- `rust-release-gate.yml` is the only workflow that runs the Rust release gate chain on Windows.
- Other workflows remain mostly Gradle/Ubuntu oriented and are not the release-gate source of truth.

## Source Monorepo Gate Snapshot (Implemented There)

### Trigger Policy

- `workflow_dispatch`
- `pull_request` with path filters for `rust/**`, `tools/**`, `fixtures/**`, `server/**`, workflow file, and Gradle wrapper/build files
- `push` on `main` and `master` with same path filters

### Runtime Model

- Job: `rust-release-gate`
- Runner: `windows-latest`
- Timeout: `60` minutes
- Permission scope: `contents: read`

### Implemented Gate Sequence

1. Checkout
2. Setup JDK 17
3. Setup Rust stable
4. Run prereq gate:
   - `powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\check-mdt-release-prereqs.ps1`
5. Run split-workspace gate:
   - `powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\verify-rust-workspaces.ps1`
6. Build local server jar:
   - `.\gradlew.bat -PnoLocalArc server:dist --stacktrace`
7. Start local server and wait for `127.0.0.1:6567`
8. Run release-set verify gate:
   - `powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\verify-mdt-client-min-release-set.ps1 -FailOnLegacyStage -Server 127.0.0.1:6567`
9. Always stop server process
10. Always upload release artifacts and server logs

### Uploaded Artifacts

- `build/windows/mdt-client-min-online-windows.zip`
- `build/windows/mdt-client-min-online-devtools.zip`
- `build/windows/mdt-client-min-online-core/**`
- `build/windows/mdt-client-min-online-devtools/**`
- `build/ci-logs/server-stdout.log`
- `build/ci-logs/server-stderr.log`

## Frozen Ownership and Policy (Track P3)

Status: `resolved`  
Frozen on: `2026-03-22`

### Owner Map (Primary + Backup)

| Scope | Primary | Backup | Responsibility |
| --- | --- | --- | --- |
| CI workflow (`.github/workflows/rust-release-gate.yml`) | `ReleaseOperator` | `ReleaseManager` | Keep trigger paths, gate order, runtime, and artifact upload policy stable. |
| Gate scripts (`tools/check-mdt-release-prereqs.ps1`, `tools/verify-rust-workspaces.ps1`, `tools/verify-mdt-client-min-release-set.ps1`) | `ReleaseOperator` | `ReleaseManager` | Maintain deterministic pass/fail markers used by CI gate decisions. |
| Localhost server smoke reliability (`127.0.0.1:6567`) | `QA` | `ReleaseManager` | Triage readiness failures, classify infra flake vs product regression, and decide rerun vs escalation. |

Repository binding:
- `CODEOWNERS` owns workflow/governance paths for review routing and change visibility.

### Required Check Policy

Protected branches (`main`, `master`) must require this check context for scoped changes:
- `Rust Release Gate / rust-release-gate`

Scoped changes (same as workflow path filters):
- `rust/**`
- `tools/**`
- `fixtures/**`
- `server/**`
- `.github/workflows/rust-release-gate.yml`
- `build.gradle`
- `settings.gradle`
- `gradle.properties`
- `gradle/**`
- `gradlew`
- `gradlew.bat`

Policy notes:
- Apply this as a path-scoped ruleset (or equivalent protected-branch policy) for the scoped-change paths above.
- The workflow/job names above are frozen to keep required-check context stable.
- Any rename of workflow or job requires same-day branch-protection update.

### Failure Handling and SLA

Triage order (do not skip order):
1. `Check release prerequisites`
2. `Verify Rust workspaces`
3. `Start localhost server and wait for 127.0.0.1:6567`
4. `Verify release set`

Response targets:
- First responder (`ReleaseOperator`): acknowledge within `30 minutes` during active release windows.
- Escalation to backup (`ReleaseManager`): if root cause is unclear after `60 minutes` or second consecutive failure.
- `QA` must classify server-readiness failures as `infra-flake` or `product-regression` before waiver discussion.

### Artifact Governance

- Upload step always runs (`if: always()`).
- Retention is fixed at `14` days.
- Mandatory reviewer: `QA` reviews `build/ci-logs/server-stdout.log` and `build/ci-logs/server-stderr.log` on failed runs.
- Storage rule: keep only the workflow artifact bundle; do not add duplicate long-term storage in this track.

### Waiver / Skip / Rerun Policy

Default:
- No skip switches in CI gate commands.
- `-SkipPreflight` and equivalent bypass flags are disallowed in this workflow.

Allowed rerun:
- Single rerun without waiver is allowed for confirmed `infra-flake` (runner/network/transient dependency).

Waiver (exception-only):
- Requires both approvals: `ReleaseManager` + `QA`.
- Must include a tracking issue/incident ID, explicit scope, and expiry (max `24h`).
- Cannot waive deterministic script failures in prereq/workspace validation.

### Closure Checklist (P3)

- [x] Ownership explicit (primary + backup)
- [x] Required-check context frozen
- [x] Failure triage/escalation SLA documented
- [x] Artifact retention/review policy documented
- [x] Waiver/skip/rerun policy documented

## Guardrail

- This gate validates the Rust `minimal compatibility client` release chain.
- This gate does not claim full Java desktop parity.
