use std::sync::Arc;

use buddy_core::{parse_tool_json, Tool, ToolError, ToolResult};
use buddy_database::Database;
use serde::Deserialize;
use serde_json::json;

/// Formats an email using the user's configured templates and records the
/// intent in the external actions log. Sending is not yet wired, so the action
/// is stored as unapproved and only a preview is returned.
pub struct SendEmailTool {
    db: Arc<Database>,
}

pub struct GitPushTool {
    db: Arc<Database>,
}

impl SendEmailTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

impl GitPushTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[derive(Debug, Deserialize)]
struct SendEmailInput {
    to: String,
    subject: String,
    body: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitPushInput {
    #[serde(default)]
    remote: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    repo_path: Option<String>,
}

impl Tool for SendEmailTool {
    fn name(&self) -> &str {
        "send_email"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: SendEmailInput = parse_tool_json(input, "send_email")?;

        let greeting = self.db.get_setting_or("email_greeting", "Hi,");
        let signature = self.db.get_setting_or("email_signature", "");
        let template = self.db.get_setting_or(
            "email_body_template",
            "{greeting}\n\n{body}\n\n{signature}",
        );

        let name = parsed.name.clone().unwrap_or_default();
        let formatted = template
            .replace("{greeting}", &greeting.replace("{name}", &name))
            .replace("{name}", &name)
            .replace("{body}", &parsed.body)
            .replace("{signature}", &signature);

        let detail = json!({
            "to": parsed.to,
            "subject": parsed.subject,
            "formatted_body": formatted,
        });
        let summary = format!("Email to {} — {}", parsed.to, parsed.subject);
        let _ = self.db.log_external_action(
            "send_email",
            &summary,
            Some(&detail.to_string()),
            false,
        );

        Ok(ToolResult {
            output: format!(
                "Email drafted (not sent — approval required). Preview:\nTo: {}\nSubject: {}\n\n{}",
                parsed.to, parsed.subject, formatted
            ),
        })
    }
}

impl Tool for GitPushTool {
    fn name(&self) -> &str {
        "git_push"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: GitPushInput = parse_tool_json(input, "git_push")?;
        let remote = parsed.remote.unwrap_or_else(|| "origin".to_string());
        let branch = parsed.branch.unwrap_or_else(|| "current branch".to_string());
        let repo = parsed.repo_path.unwrap_or_else(|| "(workspace)".to_string());

        let detail = json!({
            "remote": remote,
            "branch": branch,
            "repo_path": repo,
        });
        let summary = format!("git push {remote} {branch} in {repo}");
        let _ = self.db.log_external_action(
            "git_push",
            &summary,
            Some(&detail.to_string()),
            false,
        );

        Ok(ToolResult {
            output: format!(
                "Push to {remote}/{branch} requires approval and is not yet wired. Logged for review."
            ),
        })
    }
}
