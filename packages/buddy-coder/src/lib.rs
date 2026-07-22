//! Code Agent capability package: Codex/Cursor CLI execution and PTY
//! terminals. Planning for chat turns lives in the Brain; this package only
//! runs backends and streams results.
//!
//! The app shell wires this up via [`CodeEmit`] (streaming events) and
//! [`SecretLookup`] (Keychain-backed API keys) so this crate never depends on
//! the app's own state or secrets modules.

mod codex_runner;
mod cursor_runner;
mod execute;
mod terminal;

pub use execute::{run_code_turn, send_codex_message, CodeTurnOutcome};
pub use terminal::TerminalManager;

/// Streaming sink for a Code Agent turn. Implemented by the app shell (over
/// Tauri's `AppHandle::emit`) so the runners here stay UI-framework agnostic.
pub trait CodeEmit: Send + Sync {
    /// A chunk of assistant output to append to the live transcript.
    fn chunk(&self, text: &str);
    /// A user-facing error occurred; also treated as a final chunk.
    fn error(&self, message: &str);
    /// A local dev-server URL was detected in the agent's output.
    fn preview_url(&self, url: &str);
    /// The turn has finished (success or failure already reported via chunk/error).
    fn done(&self);
}

/// Read-only Keychain lookup, so this crate never touches secret storage
/// directly.
pub trait SecretLookup: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
}
