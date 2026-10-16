# Parking lot

Out-of-diff review observations that go-harness parked instead of turning them
into issues. A human triages this list in batches: make a story, fold into an
existing story, or delete the line.

Format: `- [<story id> #<issue>] path:line — <one sentence>` under a date heading.

## 2026-10-16

- [E1.7 #43] crates/fm-core/src/config.rs:22 — E1.6 (PR #213) `Config.telemetry: bool` and `io_error` (config.rs:96, always `ErrorCode::Io`) overlap E1.7 `TelemetrySettings` and `FmError::io`; whichever PR merges second should switch to `TelemetrySettings` and `FmError::io`/`FmResult`.
- [E1.7 #43] crates/fm-core/src/error.rs:51 — `FmError` constructors and `Display` mirror `ApiError` (crates/fm-api/src/error.rs:49) line for line; a message change on one side must be copied to the other.
