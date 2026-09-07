# Batch 4A explicit target backend review

Date: 2026-09-04
Scope: `crates/runquiry-app/src/backend.rs`, `backend/container.rs`, `backend/tests.rs`, and
`.omo/evidence/batch4a-target-backend-repro.md` in the current uncommitted worktree.

## Verdict

- `codeQualityStatus`: **WATCH**
- `recommendation`: **APPROVE**
- `blockers`: none.

## Finding

### HIGH

None. The following original HIGH finding is resolved by the re-review below.

### Resolved HIGH

1. Fresh-per-operation construction originally broke the existing cross-operation systemd single-flight
   bound. `PlatformBackend::load`, `resolve`, and `analyze` each construct a new
   `LinuxPlatform` at [backend.rs:77](../../crates/runquiry-app/src/backend.rs#L77),
   [backend.rs:143](../../crates/runquiry-app/src/backend.rs#L143), and
   [backend.rs:212](../../crates/runquiry-app/src/backend.rs#L212). Each constructor creates a
   separate `Arc<AtomicBool>` at
   [process.rs:93](../../crates/runquiry-platform/src/linux/process.rs#L93). That flag is the
   only guard passed to bounded systemd enrichment at
   [source.rs:42](../../crates/runquiry-platform/src/linux/source.rs#L42); its
   `compare_exchange` only rejects requests that share the same Arc
   ([source.rs:63](../../crates/runquiry-platform/src/linux/source.rs#L63)).

   The UI can leave an earlier analysis executing while scheduling another one: both background
   calls are started at [interactions.rs:59](../../crates/runquiry-ui/src/shell/interactions.rs#L59)
   and [interactions.rs:171](../../crates/runquiry-ui/src/shell/interactions.rs#L171), while the
   generation gate only discards stale *results*. Consequently two overlapping `analyze` calls
   now own independent flags and can each launch a D-Bus worker, defeating the Batch 3
   single-flight/timeout protection. This is a regression in a bounded external-resource path,
   not merely a performance preference.

   The current worktree resolves this with the backend-owned `AnalysisGate`; the final
   verification is recorded below.

### MEDIUM

None.

### LOW

None.

## Verified positive behavior

- The stated root cause is real: `LinuxPlatform` snapshots baseline PIDs and applies its
  construction-time exclusion policy ([process.rs:56](../../crates/runquiry-platform/src/linux/process.rs#L56),
  [process.rs:65](../../crates/runquiry-platform/src/linux/process.rs#L65),
  [process.rs:200](../../crates/runquiry-platform/src/linux/process.rs#L200)). The new backend
  no longer stores a long-lived platform, and each `load`/`resolve`/`analyze` uses one fresh
  instance consistently through the operation.
- The two real regression cases launch independent child processes, verify fresh network/file
  collection sees the child PID, verify a stale platform does not, then resolve through the
  production backend ([tests.rs:156](../../crates/runquiry-app/src/backend/tests.rs#L156),
  [tests.rs:185](../../crates/runquiry-app/src/backend/tests.rs#L185)). Their helpers use RAII
  cleanup and have no secrets in arguments or output.
- Port and file resolution now convert `Inspection.data = None` into `InspectError::Unsupported`
  at their resolution boundary instead of treating it as an empty candidate set
  ([backend.rs:153](../../crates/runquiry-app/src/backend.rs#L153),
  [backend.rs:186](../../crates/runquiry-app/src/backend.rs#L186)). The socket-owner-unknown
  container fallback and unverified-container fallback remain explicit
  ([backend.rs:163](../../crates/runquiry-app/src/backend.rs#L163),
  [container.rs:72](../../crates/runquiry-app/src/backend/container.rs#L72)).

## Independent gates

- `cargo test -p runquiry-app --locked -- --test-threads=1`: **19 passed** when run outside the
  restricted sandbox. The first sandboxed attempt could not bind the intentional
  `127.0.0.1:0` test listener (`EPERM`), so it is not treated as a product failure.
- `cargo check -p runquiry-app --locked`: pass.
- `cargo clippy -p runquiry-app --all-targets --locked -- -D warnings -A clippy::multiple_crate_versions -A clippy::print_stderr`: pass.
- `cargo fmt --all -- --check`, `git diff --check HEAD`, and `git diff --no-index --check` for
  all three backend files: pass.
- Pure LOC: `backend.rs` 249; `backend/container.rs` 99; `backend/tests.rs` 213. Production
  files satisfy the 250-line ceiling.

## Skill-perspective check

`remove-ai-slops` and `programming` perspectives were loaded and applied. No deletion-only,
tautological, implementation-constant-mirroring, brittle prompt, untyped escape-hatch, or
unnecessary parsing/normalization issue was found in this scope. The subprocess helper tests are
not counted as independent behavioral assertions; they are launched by the two real target
resolution regressions. The single-flight regression above violates the programming
resource-boundary/concurrency perspective and is the approval blocker.

## Re-review of the single-flight repair

- `PlatformBackend` owns exactly one `AnalysisGate` at
  [backend.rs:28](../../crates/runquiry-app/src/backend.rs#L28) and invokes it on the actual
  `WorkspaceBackend::analyze` path before the fresh platform is constructed
  ([backend.rs:220](../../crates/runquiry-app/src/backend.rs#L220)). `load` and `resolve` remain
  fresh-per-operation and unblocked.
- `AnalysisGate` is a RAII `MutexGuard` held only on the GPUI background executor; it cannot
  block the UI thread. Mutex poisoning is converted to the typed `InspectError::Unsupported`
  rather than a panic or silent continuation
  ([analysis_gate.rs:19](../../crates/runquiry-app/src/backend/analysis_gate.rs#L19)).
- The concurrent test creates one real `Arc<PlatformBackend>`, reaches its production
  `with_analysis_gate` sharing seam from two threads, coordinates both attempted admissions with
  a barrier, and observes a maximum active count of one
  ([tests.rs:170](../../crates/runquiry-app/src/backend/tests.rs#L170)). It is not a hand-made
  platform `AtomicBool` test. Static inspection confirms the same seam is the one used by public
  `analyze`; no deadlock path was observed in the controlled release-channel protocol.
- Fresh Port/File behavior and container fallback are unchanged at
  [backend.rs:152](../../crates/runquiry-app/src/backend.rs#L152),
  [backend.rs:195](../../crates/runquiry-app/src/backend.rs#L195), and
  [container.rs:72](../../crates/runquiry-app/src/backend/container.rs#L72).
- Independent final commands: `cargo test -p runquiry-app --locked -- --test-threads=1` (20
  passed), `cargo check -p runquiry-app --locked`, four-crate strict clippy with the two project
  exceptions, `cargo fmt --all -- --check`, and all relevant diff checks passed. Production pure
  LOC is `backend.rs` 216, `analysis_gate.rs` 23, `unavailable.rs` 49, and `container.rs` 99.

## Residual risk

Fresh construction deliberately retains the existing policy that processes born after the start
of a particular operation can be excluded as possible self descendants. The tested contract is
the intended one: an external process that already exists when the user starts a new operation is
visible. This review did not change that platform policy. The shared gate serializes only
background analyses for one backend (not refresh or resolution); a stale analysis can delay a
newer analysis, but cannot freeze the UI and preserves the bounded D-Bus behavior that motivated
the repair.
