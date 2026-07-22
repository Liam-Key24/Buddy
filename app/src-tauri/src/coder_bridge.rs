//! Wires the `buddy-coder` package's [`CodeEmit`] / [`SecretLookup`] traits to
//! this app's Tauri event bus and Keychain secrets, so the package itself
//! never depends on either.

use buddy_coder::{CodeEmit, SecretLookup};
use tauri::{AppHandle, Emitter};

/// Newtype over `AppHandle` so this crate can implement the foreign
/// `CodeEmit` trait (the orphan rule forbids `impl CodeEmit for AppHandle`
/// directly, since neither type is local to this crate).
pub struct AppEmit(pub AppHandle);

impl CodeEmit for AppEmit {
    fn chunk(&self, text: &str) {
        let _ = self.0.emit("codex-chunk", text);
    }

    fn error(&self, message: &str) {
        let _ = self.0.emit("codex-error", message);
        let _ = self.0.emit("codex-chunk", message);
    }

    fn preview_url(&self, url: &str) {
        let _ = self.0.emit("code-preview-url", url);
    }

    fn done(&self) {
        let _ = self.0.emit("codex-done", ());
    }
}

/// Streams a Code Agent turn into Buddy chat's event channel.
/// Retained for optional live streaming; `coder.run` currently uses CollectEmit.
#[allow(dead_code)]
pub struct ChatEmit(pub AppHandle);

impl CodeEmit for ChatEmit {
    fn chunk(&self, text: &str) {
        let _ = self.0.emit("chat-chunk", text);
    }

    fn error(&self, message: &str) {
        let _ = self.0.emit("chat-chunk", message);
    }

    fn preview_url(&self, url: &str) {
        let _ = self.0.emit("chat-chunk", format!("\nPreview: {url}\n"));
    }

    fn done(&self) {
        // No-op: Buddy's `send_message` emits the single `chat-done`
        // for this turn only after persisting the assistant message, since
        // the frontend's `chat-done` listener is one-shot.
    }
}

pub struct KeychainSecrets;

impl SecretLookup for KeychainSecrets {
    fn get(&self, key: &str) -> Option<String> {
        crate::secrets::get_secret(key).ok().flatten()
    }
}
