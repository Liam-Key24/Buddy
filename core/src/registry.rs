use std::collections::HashMap;
use std::sync::Arc;

use crate::error::ToolError;
use crate::tool::Tool;

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
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
