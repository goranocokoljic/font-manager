//! Font Manager platform layer.
//!
//! The one place with OS-specific code: the `FontActivator` trait with one
//! implementation per OS under `src/{windows,macos,linux}/`, the app-paths
//! helpers, the tray and the IPC transport. Platform differences are
//! documented in an ADR, never hidden in `cfg` branches elsewhere.
