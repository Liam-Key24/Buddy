use std::sync::Arc;

use buddy_database::Database;

use crate::schema::ToolSchema;
use crate::tool::Tool;

/// One planner-facing entry describing a tool a plugin contributes. Used to
/// build the Brain's "available tools" list without hand-syncing a prompt
/// string against the Rust registry.
#[derive(Debug, Clone, Copy)]
pub struct ToolDecl {
    pub name: &'static str,
    /// A single line describing the tool and its `tool_input` shape, suitable
    /// for direct inclusion in the planner system prompt.
    pub planner_line: &'static str,
}

/// A default setting a plugin wants seeded on first run.
#[derive(Debug, Clone, Copy)]
pub struct SettingSeed {
    pub key: &'static str,
    pub value: &'static str,
}

/// Side effects the orchestrator should perform after a tool from this
/// plugin runs successfully, without the orchestrator needing to know the
/// tool's name in advance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AfterExecute {
    #[default]
    None,
    /// Re-emit the `sparks-stale` / `sparks-updated` events (spark tools).
    EmitSparksUpdated,
    /// Re-emit `calendar-updated` so the Calendar UI reloads (calendar tools).
    EmitCalendarUpdated,
}

/// A self-contained capability bundle: the tools it exposes to the chat
/// planner, plus the settings/secrets it needs seeded so the app shell
/// doesn't have to know about them individually.
pub trait BuddyPlugin: Send + Sync {
    fn id(&self) -> &'static str;

    /// Builds this plugin's tools, wired to the shared database handle.
    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>>;

    /// Planner-facing tool descriptions. Empty by default for plugins with no
    /// chat-callable tools.
    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[]
    }

    /// Clarification schemas for tools this plugin contributes. Empty by default.
    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[]
    }

    /// Default settings to seed on first run.
    fn setting_seeds(&self) -> &'static [SettingSeed] {
        &[]
    }

    /// Keychain secret keys the Settings UI should allow managing.
    fn secret_keys(&self) -> &'static [&'static str] {
        &[]
    }

    /// Orchestrator hint for what to do after a successful run of `tool_name`.
    fn after_execute_hint(&self, _tool_name: &str) -> AfterExecute {
        AfterExecute::None
    }
}
