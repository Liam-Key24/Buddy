use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::error::ToolError;
use crate::tool::Tool;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    disabled: Mutex<HashSet<String>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            disabled: Mutex::new(HashSet::new()),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn disable(&self, name: &str) {
        if let Ok(mut set) = self.disabled.lock() {
            set.insert(name.to_string());
        }
    }

    pub fn is_disabled(&self, name: &str) -> bool {
        self.disabled
            .lock()
            .map(|s| s.contains(name))
            .unwrap_or(false)
    }

    pub fn get(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        if self.is_disabled(name) {
            return Err(ToolError::ExecutionFailed(format!(
                "tool `{name}` is disabled after a failure"
            )));
        }
        self.tools
            .get(name)
            .cloned()
            .ok_or_else(|| ToolError::NotFound(name.to_string()))
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
