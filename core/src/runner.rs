use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Instant;

use tracing::{error, info, instrument};

use crate::error::{ToolError, ToolResult};
use crate::registry::ToolRegistry;

pub struct TaskRunner {
    registry: Arc<ToolRegistry>,
}

impl TaskRunner {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    #[instrument(skip(self), fields(tool = %name))]
    pub fn run(&self, name: &str, input: &str) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let tool = self.registry.get(name)?;
        let result = catch_unwind(AssertUnwindSafe(|| tool.execute(input)));
        match result {
            Ok(Ok(out)) => {
                info!(
                    duration_ms = start.elapsed().as_millis() as u64,
                    "tool execution complete"
                );
                Ok(out)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                error!(tool = %name, "tool panicked — isolated");
                self.registry.disable(name);
                Err(ToolError::ExecutionFailed(format!(
                    "tool `{name}` panicked and has been disabled for this session"
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ToolResult;
    use crate::tool::Tool;

    struct BoomTool;
    impl Tool for BoomTool {
        fn name(&self) -> &str {
            "boom"
        }
        fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
            panic!("plugin crash");
        }
    }

    struct OkTool;
    impl Tool for OkTool {
        fn name(&self) -> &str {
            "ok"
        }
        fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
            Ok(ToolResult {
                output: input.to_string(),
            })
        }
    }

    #[test]
    fn panic_disables_tool_and_others_continue() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(BoomTool));
        registry.register(Arc::new(OkTool));
        let runner = TaskRunner::new(Arc::new(registry));

        let err = runner.run("boom", "{}").unwrap_err();
        assert!(err.to_string().contains("panicked"));

        let again = runner.run("boom", "{}").unwrap_err();
        assert!(again.to_string().contains("disabled"));

        let ok = runner.run("ok", "still-works").unwrap();
        assert_eq!(ok.output, "still-works");
    }
}
