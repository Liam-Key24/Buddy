//! Cool Mode defaults for every production model and tool path.
//!
//! One typed policy owned by the control plane. Do not scatter these limits.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Authoritative Cool Mode / advanced-override limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePolicy {
    pub max_model_calls: u32,
    pub max_tool_executions: u32,
    pub max_repeated_tool: u32,
    pub qwen_max_tokens: u32,
    pub llama_max_tokens: u32,
    pub model_timeout: Duration,
    pub turn_deadline: Duration,
    pub mlx_idle: Duration,
    pub keep_warm: bool,
    pub thinking_enabled: bool,
    /// Cool Mode never starts a second model after Llama or Qwen fails.
    pub allows_model_fallback: bool,
}

impl RuntimePolicy {
    /// Safe defaults. Advanced settings may override later.
    pub fn cool() -> Self {
        Self {
            max_model_calls: 3,
            max_tool_executions: 8,
            max_repeated_tool: 1,
            qwen_max_tokens: 1024,
            llama_max_tokens: 256,
            model_timeout: Duration::from_secs(15),
            turn_deadline: Duration::from_secs(60),
            mlx_idle: Duration::from_secs(45),
            keep_warm: false,
            thinking_enabled: false,
            allows_model_fallback: false,
        }
    }

    pub fn tokens_for_qwen(&self) -> u32 {
        self.qwen_max_tokens
    }

    pub fn tokens_for_llama(&self) -> u32 {
        self.llama_max_tokens
    }
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self::cool()
    }
}

/// Normalize tool arguments so equivalent JSON compares equal.
pub fn normalize_tool_args(input: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(input.trim()) {
        Ok(value) => canonical_json(&value),
        Err(_) => input.trim().to_string(),
    }
}

fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner = keys
                .into_iter()
                .map(|k| format!("{}:{}", serde_json::to_string(k).unwrap_or_default(), canonical_json(&map[k])))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{inner}}}")
        }
        serde_json::Value::Array(items) => {
            let inner = items.iter().map(canonical_json).collect::<Vec<_>>().join(",");
            format!("[{inner}]")
        }
        other => other.to_string(),
    }
}

pub fn tool_fingerprint(name: &str, input: &str) -> String {
    format!("{name}\n{}", normalize_tool_args(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cool_defaults_match_spec() {
        let p = RuntimePolicy::cool();
        assert_eq!(p.max_model_calls, 3);
        assert_eq!(p.max_tool_executions, 8);
        assert_eq!(p.max_repeated_tool, 1);
        assert_eq!(p.qwen_max_tokens, 1024);
        assert_eq!(p.llama_max_tokens, 256);
        assert_eq!(p.model_timeout, Duration::from_secs(15));
        assert_eq!(p.turn_deadline, Duration::from_secs(60));
        assert_eq!(p.mlx_idle, Duration::from_secs(45));
        assert!(!p.keep_warm);
        assert!(!p.thinking_enabled);
        assert!(!p.allows_model_fallback);
    }

    #[test]
    fn equivalent_json_args_share_a_fingerprint() {
        let a = tool_fingerprint("money.log", r#"{"amount":10,"note":"x"}"#);
        let b = tool_fingerprint("money.log", r#"{"note":"x","amount":10}"#);
        assert_eq!(a, b);
        assert_ne!(a, tool_fingerprint("money.log", r#"{"amount":11,"note":"x"}"#));
    }
}
