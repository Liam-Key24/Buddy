//! One typed policy owned by the control plane. Pass it through the turn.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Authoritative inference / Cool Mode limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePolicy {
    pub max_model_calls: u32,
    pub max_tool_executions: u32,
    pub max_repeated_tool: u32,
    pub qwen_interpret_tokens: u32,
    pub qwen_chat_tokens: u32,
    pub llama_max_tokens: u32,
    pub startup_deadline: Duration,
    pub warm_generation: Duration,
    pub cold_first_generation: Duration,
    pub turn_deadline_after_ready: Duration,
    pub mlx_idle: Duration,
    pub keep_warm: bool,
    pub thinking_enabled: bool,
    pub allows_model_fallback: bool,
    pub llama_chat_cache: bool,
}

impl RuntimePolicy {
    pub fn cool() -> Self {
        Self {
            max_model_calls: 2,
            max_tool_executions: 8,
            max_repeated_tool: 1,
            qwen_interpret_tokens: 512,
            qwen_chat_tokens: 256,
            llama_max_tokens: 256,
            startup_deadline: Duration::from_secs(90),
            warm_generation: Duration::from_secs(120),
            cold_first_generation: Duration::from_secs(180),
            turn_deadline_after_ready: Duration::from_secs(180),
            mlx_idle: Duration::from_secs(300),
            keep_warm: false,
            thinking_enabled: false,
            allows_model_fallback: false,
            llama_chat_cache: false,
        }
    }

    pub fn tokens_for_qwen(&self) -> u32 {
        self.qwen_interpret_tokens
    }

    pub fn tokens_for_llama(&self) -> u32 {
        self.llama_max_tokens
    }

    pub fn turn_deadline(&self) -> Duration {
        self.turn_deadline_after_ready
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
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonical_json(&map[k])
                    )
                })
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

pub fn action_idempotency_key(turn_id: &str, index: u32) -> String {
    format!("{turn_id}:{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cool_defaults_match_spec() {
        let p = RuntimePolicy::cool();
        assert_eq!(p.max_model_calls, 2);
        assert_eq!(p.max_tool_executions, 8);
        assert_eq!(p.max_repeated_tool, 1);
        assert_eq!(p.qwen_interpret_tokens, 512);
        assert_eq!(p.qwen_chat_tokens, 256);
        assert_eq!(p.llama_max_tokens, 256);
        assert_eq!(p.startup_deadline, Duration::from_secs(90));
        assert_eq!(p.warm_generation, Duration::from_secs(120));
        assert_eq!(p.cold_first_generation, Duration::from_secs(180));
        assert_eq!(p.turn_deadline_after_ready, Duration::from_secs(180));
        assert_eq!(p.mlx_idle, Duration::from_secs(300));
        assert!(!p.keep_warm);
        assert!(!p.thinking_enabled);
        assert!(!p.allows_model_fallback);
        assert!(!p.llama_chat_cache);
        assert_ne!(p.startup_deadline, p.warm_generation);
        assert_ne!(p.warm_generation, Duration::from_secs(15));
    }

    #[test]
    fn equivalent_json_args_share_a_fingerprint() {
        let a = tool_fingerprint("money.log", r#"{"amount":10,"note":"x"}"#);
        let b = tool_fingerprint("money.log", r#"{"note":"x","amount":10}"#);
        assert_eq!(a, b);
        assert_ne!(
            a,
            tool_fingerprint("money.log", r#"{"amount":11,"note":"x"}"#)
        );
    }

    #[test]
    fn idempotency_key_is_turn_plus_index() {
        assert_eq!(action_idempotency_key("turn-a", 0), "turn-a:0");
        assert_ne!(action_idempotency_key("turn-a", 0), action_idempotency_key("turn-a", 1));
    }
}
