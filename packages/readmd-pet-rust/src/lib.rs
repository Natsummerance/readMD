//! Production ReadMD desktop-pet host.
//!
//! The GUI and WebView are deliberately owned by the Tao event-loop thread.
//! File polling, parent liveness, and IPC decoding run in small background
//! workers and return typed events through Tao's user-event proxy.

pub mod bridge;
pub mod clipboard;
pub mod error;
pub mod input;
pub mod platform;
pub mod protocol;
pub mod runtime;
pub mod security;
pub mod webview;

pub use error::{HostError, HostResult};
pub use protocol::{ClipboardCommand, CommandEnvelope, PetSnapshot, RendererKind, SnapshotBounds};
pub use runtime::{HostConfig, PetHost};
