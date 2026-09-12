use std::sync::Arc;

use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, SettingSeed, Tool, ToolDecl,
    ToolError, ToolResult, ToolSchema, ToolSpec,
};
use buddy_database::{local_today, Database, MoneyEntry, MoneyPot};
use serde::Deserialize;

pub struct MoneyPlugin;

struct SummaryTool { db: Arc<Database> }
struct ListTool { db: Arc<Database> }
struct AnalyzeTool { db: Arc<Database> }
struct LogTool { db: Arc<Database> }
struct PotsTool { db: Arc<Database> }
struct PotTool { db: Arc<Database> }

const MONEY_SPECS: &[ToolSpec] = &[
            ToolSpec::basic("money.summary", "income/expense totals for a month", r#"money.summary"#, buddy_core::empty_schema("money.summary")),
            ToolSpec::basic("money.list", "ledger rows for a month", r#"money.list kind=expense"#, buddy_core::empty_schema("money.list")),
            ToolSpec::basic("money.analyze", "biggest expenses and month-over-month changes", r#"money.analyze"#, buddy_core::empty_schema("money.analyze")),
            ToolSpec::basic("money.log", "record income or an expense", r#"money.log kind=expense description=lunch amount=12.50"#, buddy_core::empty_schema("money.log")),
            ToolSpec::basic("money.pots", "list savings pots", r#"money.pots"#, buddy_core::empty_schema("money.pots")),
            ToolSpec::basic("money.pot", "set or add to a named savings pot", r#"money.pot name=holiday amount=200"#, buddy_core::empty_schema("money.pot")),
        ];

impl BuddyPlugin for MoneyPlugin {
    fn id(&self) -> &'static str { "money" }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(SummaryTool { db: db.clone() }),
            Arc::new(ListTool { db: db.clone() }),
            Arc::new(AnalyzeTool { db: db.clone() }),
            Arc::new(LogTool { db: db.clone() }),
            Arc::new(PotsTool { db: db.clone() }),
            Arc::new(PotTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl { name: "money.summary", planner_line: "money.summary: income/expense totals for a month. tool_input JSON: {\"year?\":2026, \"month?\":8}" },
            ToolDecl { name: "money.list", planner_line: "money.list: ledger rows for a month. tool_input JSON: {\"year?\":2026, \"month?\":8, \"kind?\":\"expense|income\"}. Use when they ask what they spent or earned." },
            ToolDecl { name: "money.analyze", planner_line: "money.analyze: biggest expenses, recurring costs, month-over-month changes. Suggestions only — do not make financial decisions. tool_input JSON: {\"year?\":2026, \"month?\":8}" },
            ToolDecl { name: "money.log", planner_line: "money.log: record income or an expense. tool_input JSON: {\"kind\":\"expense|income\", \"description\":\"lunch\", \"amount\":12.50, \"amount_cents?\":1250, \"category?\":\"food\", \"date?\":\"YYYY-MM-DD\"}. Prefer amount in pounds (12.50 → £12.50). Personal ledger — not work sales." },
            ToolDecl { name: "money.pots", planner_line: "money.pots: list savings pots and the split. tool_input JSON: {}" },
            ToolDecl { name: "money.pot", planner_line: "money.pot: set or add to a named savings pot. tool_input JSON: {\"name\":\"holiday\", \"amount\":200, \"mode?\":\"set|add\"}. mode=set replaces the balance (split dump). mode=add puts more in (put £50 in holiday)." },
        ]
    }

    fn tool_specs(&self) -> &'static [ToolSpec] {
        MONEY_SPECS
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[
            ToolSchema {
                tool: "money.summary",
                fields: &[FieldSpec { name: "month", label: "month", required: false, memory_keys: &["preferred_currency"], ask_kind: AskKind::Text, choices: &[] }],
            },
            ToolSchema {
                tool: "money.log",
                fields: &[
                    FieldSpec { name: "kind", label: "income or expense", required: true, memory_keys: &[], ask_kind: AskKind::Text, choices: &[] },
                    FieldSpec { name: "description", label: "what", required: true, memory_keys: &[], ask_kind: AskKind::Text, choices: &[] },
                    FieldSpec { name: "amount", label: "pounds", required: true, memory_keys: &[], ask_kind: AskKind::Text, choices: &[] },
                ],
            },
        ]
    }

    fn setting_seeds(&self) -> &'static [SettingSeed] {
        &[SettingSeed { key: "money_currency", value: "GBP" }]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        if tool_name == "money.log" || tool_name == "money.pot" {
            AfterExecute::EmitMoneyUpdated
        } else {
            AfterExecute::None
        }
    }
}

#[derive(Deserialize, Default)]
struct MonthIn {
    #[serde(default)] year: Option<i32>,
    #[serde(default)] month: Option<i32>,
}

#[derive(Deserialize)]
struct LogIn {
    kind: String,
    description: String,
    #[serde(default)] amount: Option<f64>,
    #[serde(default)] amount_cents: Option<i64>,
    #[serde(default)] category: Option<String>,
    #[serde(default)] date: Option<String>,
}

fn resolve_ym(parsed: &MonthIn) -> (i32, i32) {
    if let (Some(y), Some(m)) = (parsed.year, parsed.month) {
        return (y, m);
    }
    let today = local_today();
    let parts: Vec<i32> = today.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        (2026, 1)
    }
}

pub fn pounds_to_cents(pounds: f64) -> i64 {
    (pounds.abs() * 100.0).round() as i64
}

fn resolve_cents(amount: Option<f64>, amount_cents: Option<i64>) -> Result<i64, ToolError> {
    if let Some(c) = amount_cents.filter(|c| *c != 0) {
        return Ok(c.abs());
    }
    if let Some(a) = amount.filter(|a| *a != 0.0) {
        return Ok(pounds_to_cents(a));
    }
    Err(ToolError::ExecutionFailed(
        "amount required (pounds like 12.50, or amount_cents)".into(),
    ))
}

impl Tool for SummaryTool {
    fn name(&self) -> &str { "money.summary" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: MonthIn = parse_tool_json(input, "money.summary").unwrap_or_default();
        let (y, m) = resolve_ym(&parsed);
        let s = self.db.money_month_summary(y, m).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult { output: serde_json::to_string_pretty(&s).unwrap_or_default() })
    }
}

#[derive(Deserialize, Default)]
struct ListIn {
    #[serde(default)] year: Option<i32>,
    #[serde(default)] month: Option<i32>,
    #[serde(default)] kind: Option<String>,
}

impl Tool for ListTool {
    fn name(&self) -> &str { "money.list" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ListIn = parse_tool_json(input, "money.list").unwrap_or_default();
        let (y, m) = resolve_ym(&MonthIn { year: parsed.year, month: parsed.month });
        let kind = parsed.kind.map(|k| k.to_ascii_lowercase()).filter(|k| k == "income" || k == "expense");
        let entries: Vec<_> = self
            .db
            .list_money_entries(y, m)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .into_iter()
            .filter(|e| kind.as_ref().map(|k| e.kind == *k).unwrap_or(true))
            .map(|e| serde_json::json!({
                "id": e.id,
                "kind": e.kind,
                "date": e.date,
                "description": e.description,
                "category": e.category,
                "amount_cents": e.amount_cents,
                "amount": e.amount_cents as f64 / 100.0,
            }))
            .collect();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&serde_json::json!({
                "year": y,
                "month": m,
                "entries": entries,
            })).unwrap_or_default(),
        })
    }
}

impl Tool for AnalyzeTool {
    fn name(&self) -> &str { "money.analyze" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: MonthIn = parse_tool_json(input, "money.analyze").unwrap_or_default();
        let (y, m) = resolve_ym(&parsed);
        let a = self.db.analyze_money(y, m).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult { output: serde_json::to_string_pretty(&a).unwrap_or_default() })
    }
}

#[derive(Deserialize)]
struct PotIn {
    name: String,
    #[serde(default)] amount: Option<f64>,
    #[serde(default)] amount_cents: Option<i64>,
    #[serde(default)] mode: Option<String>,
}

fn pot_json(p: &MoneyPot) -> serde_json::Value {
    serde_json::json!({
        "id": p.id,
        "name": p.name,
        "slug": p.slug,
        "balance_cents": p.balance_cents,
        "amount": p.balance_cents as f64 / 100.0,
    })
}

impl Tool for PotsTool {
    fn name(&self) -> &str { "money.pots" }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let pots = self
            .db
            .list_money_pots()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let total: i64 = pots.iter().map(|p| p.balance_cents).sum();
        let shares: Vec<_> = pots
            .iter()
            .map(|p| {
                let mut v = pot_json(p);
                v["pct"] = serde_json::json!(if total > 0 {
                    p.balance_cents as f64 / total as f64 * 100.0
                } else {
                    0.0
                });
                v
            })
            .collect();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&serde_json::json!({
                "pots": shares,
                "pots_cents": total,
                "amount": total as f64 / 100.0,
            }))
            .unwrap_or_default(),
        })
    }
}

impl Tool for PotTool {
    fn name(&self) -> &str { "money.pot" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: PotIn = parse_tool_json(input, "money.pot")?;
        let cents = resolve_cents(p.amount, p.amount_cents)?;
        let mode = p
            .mode
            .as_deref()
            .unwrap_or("set")
            .to_ascii_lowercase();
        let mode = if mode == "add" { "add" } else { "set" };
        let pot = self
            .db
            .apply_money_pot(&p.name, cents, mode)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let mut out = pot_json(&pot);
        out["mode"] = serde_json::json!(mode);
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&out).unwrap_or_default(),
        })
    }
}

impl Tool for LogTool {
    fn name(&self) -> &str { "money.log" }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: LogIn = parse_tool_json(input, "money.log")?;
        let kind = p.kind.to_ascii_lowercase();
        if kind != "income" && kind != "expense" {
            return Err(ToolError::ExecutionFailed("kind must be income or expense".into()));
        }
        let cents = resolve_cents(p.amount, p.amount_cents)?;
        let date = p.date.filter(|d| !d.is_empty()).unwrap_or_else(local_today);
        let entry = self.db.upsert_money_entry(MoneyEntry {
            id: String::new(),
            kind,
            date,
            description: p.description,
            category: p.category.filter(|c| !c.is_empty()).unwrap_or_else(|| "general".into()),
            amount_cents: cents,
            year: 0,
            month: 0,
            created_at: 0,
            updated_at: 0,
        }).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult { output: serde_json::to_string_pretty(&entry).unwrap_or_default() })
    }
}

#[cfg(test)]
mod tests {
    use super::pounds_to_cents;

    #[test]
    fn pounds_convert() {
        assert_eq!(pounds_to_cents(12.5), 1250);
        assert_eq!(pounds_to_cents(12.50), 1250);
        assert_eq!(pounds_to_cents(0.99), 99);
    }
}
