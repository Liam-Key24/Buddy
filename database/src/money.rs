use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{chrono_now, Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyEntry {
    pub id: String,
    pub kind: String,
    pub date: String,
    pub description: String,
    pub category: String,
    pub amount_cents: i64,
    pub year: i32,
    pub month: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyPot {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub balance_cents: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyPotShare {
    pub id: String,
    pub name: String,
    pub balance_cents: i64,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyMonthSummary {
    pub year: i32,
    pub month: i32,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub net_cents: i64,
    pub savings_cents: i64,
    pub by_category: Vec<MoneyCategoryTotal>,
    #[serde(default)]
    pub pots: Vec<MoneyPotShare>,
    #[serde(default)]
    pub pots_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyCategoryTotal {
    pub category: String,
    pub kind: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyAnalysis {
    pub summary: MoneyMonthSummary,
    pub previous: Option<MoneyMonthSummary>,
    pub biggest_expenses: Vec<MoneyEntry>,
    pub recurring: Vec<String>,
    pub notes: Vec<String>,
}

impl Database {
    pub fn list_money_entries(&self, year: i32, month: i32) -> Result<Vec<MoneyEntry>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, kind, date, description, category, amount_cents, year, month, created_at, updated_at
                 FROM money_entries WHERE year=?1 AND month=?2 ORDER BY date, created_at",
            )?;
            let rows = stmt.query_map(params![year, month], map_money)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_money_entry(&self, entry: MoneyEntry) -> Result<MoneyEntry, DbError> {
        let now = chrono_now();
        let mut e = entry;
        if e.id.is_empty() {
            e.id = Uuid::new_v4().to_string();
            e.created_at = now;
        }
        e.updated_at = now;
        if e.year == 0 || e.month == 0 {
            if let Some((y, m, _)) = parse_date_parts(&e.date) {
                e.year = y;
                e.month = m;
            }
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO money_entries (id, kind, date, description, category, amount_cents, year, month, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
                 ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, date=excluded.date, description=excluded.description,
                    category=excluded.category, amount_cents=excluded.amount_cents, year=excluded.year, month=excluded.month,
                    updated_at=excluded.updated_at",
                params![e.id, e.kind, e.date, e.description, e.category, e.amount_cents, e.year, e.month, e.created_at, e.updated_at],
            )?;
            Ok(())
        })?;
        Ok(e)
    }

    pub fn delete_money_entry(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM money_entries WHERE id=?1", params![id])?;
            if n == 0 {
                Err(DbError::NotFound(id.into()))
            } else {
                Ok(())
            }
        })
    }

    pub fn money_month_summary(&self, year: i32, month: i32) -> Result<MoneyMonthSummary, DbError> {
        let entries = self.list_money_entries(year, month)?;
        let mut summary = summarize_month(year, month, &entries);
        attach_pots(&mut summary, &self.list_money_pots()?);
        Ok(summary)
    }

    pub fn list_money_pots(&self) -> Result<Vec<MoneyPot>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, slug, balance_cents, created_at, updated_at
                 FROM money_pots ORDER BY name COLLATE NOCASE",
            )?;
            let rows = stmt.query_map([], map_pot)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_money_pot(&self, pot: MoneyPot) -> Result<MoneyPot, DbError> {
        let now = chrono_now();
        let mut p = pot;
        p.slug = pot_slug(&p.name);
        if p.slug.is_empty() {
            p.slug = "savings".into();
        }
        if p.name.trim().is_empty() {
            p.name = display_pot_name(&p.slug);
        }
        if p.id.is_empty() {
            if let Some(existing) = self.find_money_pot_by_slug(&p.slug)? {
                p.id = existing.id;
                p.created_at = existing.created_at;
            } else {
                p.id = Uuid::new_v4().to_string();
                p.created_at = now;
            }
        }
        p.updated_at = now;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO money_pots (id, name, slug, balance_cents, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, slug=excluded.slug,
                    balance_cents=excluded.balance_cents, updated_at=excluded.updated_at",
                params![p.id, p.name, p.slug, p.balance_cents, p.created_at, p.updated_at],
            )?;
            Ok(())
        })?;
        Ok(p)
    }

    pub fn apply_money_pot(&self, name: &str, amount_cents: i64, mode: &str) -> Result<MoneyPot, DbError> {
        let mut slug = pot_slug(name);
        if slug.is_empty() {
            slug = "savings".into();
        }
        let existing = self.find_money_pot_by_slug(&slug)?;
        let mut pot = existing.unwrap_or(MoneyPot {
            id: String::new(),
            name: display_pot_name(name),
            slug: slug.clone(),
            balance_cents: 0,
            created_at: 0,
            updated_at: 0,
        });
        if !name.trim().is_empty() {
            pot.name = display_pot_name(name);
        }
        if mode.eq_ignore_ascii_case("add") {
            pot.balance_cents += amount_cents.abs();
        } else {
            pot.balance_cents = amount_cents.abs();
        }
        self.upsert_money_pot(pot)
    }

    pub fn delete_money_pot(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM money_pots WHERE id=?1", params![id])?;
            if n == 0 {
                Err(DbError::NotFound(id.into()))
            } else {
                Ok(())
            }
        })
    }

    fn find_money_pot_by_slug(&self, slug: &str) -> Result<Option<MoneyPot>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, slug, balance_cents, created_at, updated_at
                 FROM money_pots WHERE slug=?1",
            )?;
            let mut rows = stmt.query_map(params![slug], map_pot)?;
            rows.next().transpose().map_err(DbError::from)
        })
    }

    pub fn analyze_money(&self, year: i32, month: i32) -> Result<MoneyAnalysis, DbError> {
        let entries = self.list_money_entries(year, month)?;
        let mut summary = summarize_month(year, month, &entries);
        let pots = self.list_money_pots()?;
        attach_pots(&mut summary, &pots);
        let (py, pm) = prev_month(year, month);
        let prev_entries = self.list_money_entries(py, pm).unwrap_or_default();
        let previous = if prev_entries.is_empty() {
            None
        } else {
            let mut prev = summarize_month(py, pm, &prev_entries);
            attach_pots(&mut prev, &pots);
            Some(prev)
        };
        Ok(analyze_money_data(summary, previous, &entries, &prev_entries))
    }

    pub fn format_money_digest(&self) -> Option<String> {
        let today = crate::local_today();
        let (y, m, _) = parse_date_parts(&today)?;
        let s = self.money_month_summary(y, m).ok()?;
        let top = s
            .by_category
            .iter()
            .filter(|c| c.kind == "expense")
            .max_by_key(|c| c.amount_cents)
            .map(|c| c.category.as_str())
            .unwrap_or("—");
        let pots = if s.pots.is_empty() {
            String::new()
        } else {
            let split = s
                .pots
                .iter()
                .map(|p| format!("{} £{:.2}", p.name, p.balance_cents as f64 / 100.0))
                .collect::<Vec<_>>()
                .join(", ");
            format!(" Savings pots: {split}.")
        };
        Some(format!(
            "Money {y}-{m:02}: income £{:.2}, expenses £{:.2}, net £{:.2}. Top expense category: {top}.{pots}",
            s.income_cents as f64 / 100.0,
            s.expense_cents as f64 / 100.0,
            s.net_cents as f64 / 100.0
        ))
    }
}

fn map_money(row: &rusqlite::Row<'_>) -> Result<MoneyEntry, rusqlite::Error> {
    Ok(MoneyEntry {
        id: row.get(0)?,
        kind: row.get(1)?,
        date: row.get(2)?,
        description: row.get(3)?,
        category: row.get(4)?,
        amount_cents: row.get(5)?,
        year: row.get(6)?,
        month: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn parse_date_parts(date: &str) -> Option<(i32, i32, i32)> {
    let p: Vec<i32> = date.split('-').filter_map(|x| x.parse().ok()).collect();
    if p.len() == 3 {
        Some((p[0], p[1], p[2]))
    } else {
        None
    }
}

fn prev_month(year: i32, month: i32) -> (i32, i32) {
    if month <= 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

pub fn summarize_month(year: i32, month: i32, entries: &[MoneyEntry]) -> MoneyMonthSummary {
    let mut income = 0i64;
    let mut expense = 0i64;
    let mut savings = 0i64;
    let mut cats: Vec<MoneyCategoryTotal> = Vec::new();
    for e in entries {
        if e.kind == "income" {
            income += e.amount_cents;
        } else {
            expense += e.amount_cents;
            if e.category.eq_ignore_ascii_case("savings") {
                savings += e.amount_cents;
            }
        }
        if let Some(existing) = cats
            .iter_mut()
            .find(|c| c.category == e.category && c.kind == e.kind)
        {
            existing.amount_cents += e.amount_cents;
        } else {
            cats.push(MoneyCategoryTotal {
                category: e.category.clone(),
                kind: e.kind.clone(),
                amount_cents: e.amount_cents,
            });
        }
    }
    cats.sort_by(|a, b| b.amount_cents.cmp(&a.amount_cents));
    MoneyMonthSummary {
        year,
        month,
        income_cents: income,
        expense_cents: expense,
        net_cents: income - expense,
        savings_cents: if savings > 0 { savings } else { income - expense },
        by_category: cats,
        pots: Vec::new(),
        pots_cents: 0,
    }
}

fn attach_pots(summary: &mut MoneyMonthSummary, pots: &[MoneyPot]) {
    let total: i64 = pots.iter().map(|p| p.balance_cents).sum();
    summary.pots_cents = total;
    summary.pots = pots
        .iter()
        .map(|p| MoneyPotShare {
            id: p.id.clone(),
            name: p.name.clone(),
            balance_cents: p.balance_cents,
            pct: if total > 0 {
                (p.balance_cents as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect();
    if total > 0 {
        summary.savings_cents = total;
    }
}

fn map_pot(row: &rusqlite::Row<'_>) -> Result<MoneyPot, rusqlite::Error> {
    Ok(MoneyPot {
        id: row.get(0)?,
        name: row.get(1)?,
        slug: row.get(2)?,
        balance_cents: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub fn pot_slug(name: &str) -> String {
    name.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| {
            !w.is_empty()
                && !matches!(
                    *w,
                    "pot" | "pots" | "the" | "my" | "a" | "an"
                )
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn display_pot_name(name: &str) -> String {
    let cleaned = name
        .split_whitespace()
        .filter(|w| {
            let l = w.to_ascii_lowercase();
            !matches!(l.as_str(), "pot" | "pots" | "the" | "my")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Savings".into()
    } else {
        let mut chars = cleaned.chars();
        match chars.next() {
            Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
            None => "Savings".into(),
        }
    }
}

pub fn analyze_money_data(
    summary: MoneyMonthSummary,
    previous: Option<MoneyMonthSummary>,
    entries: &[MoneyEntry],
    prev_entries: &[MoneyEntry],
) -> MoneyAnalysis {
    let mut biggest: Vec<MoneyEntry> = entries
        .iter()
        .filter(|e| e.kind == "expense")
        .cloned()
        .collect();
    biggest.sort_by(|a, b| b.amount_cents.cmp(&a.amount_cents));
    biggest.truncate(5);

    let mut recurring = Vec::new();
    for e in entries.iter().filter(|e| e.kind == "expense") {
        let key = e.description.to_lowercase();
        if prev_entries.iter().any(|p| {
            p.kind == "expense" && p.description.to_lowercase() == key
        }) && !recurring.iter().any(|r: &String| r == &e.description)
        {
            recurring.push(e.description.clone());
        }
    }

    let mut notes = Vec::new();
    if let Some(prev) = &previous {
        for cat in summary.by_category.iter().filter(|c| c.kind == "expense") {
            if let Some(old) = prev
                .by_category
                .iter()
                .find(|c| c.kind == "expense" && c.category == cat.category)
            {
                if old.amount_cents > 0 {
                    let delta = (cat.amount_cents - old.amount_cents) as f64 / old.amount_cents as f64;
                    if delta >= 0.2 {
                        notes.push(format!(
                            "You spent {:.0}% more on {} this month than last month.",
                            delta * 100.0,
                            cat.category
                        ));
                    }
                }
            }
        }
        let subs: i64 = entries
            .iter()
            .filter(|e| {
                e.kind == "expense"
                    && (e.category.eq_ignore_ascii_case("subscriptions")
                        || e.description.to_lowercase().contains("subscription"))
            })
            .map(|e| e.amount_cents)
            .sum();
        if subs > 0 {
            notes.push(format!(
                "Subscriptions currently cost £{:.2}/month. These may be an area where you could reduce spending.",
                subs as f64 / 100.0
            ));
        }
    }
    if notes.is_empty() {
        notes.push("Suggestions are based only on the figures you entered.".into());
    }

    MoneyAnalysis {
        summary,
        previous,
        biggest_expenses: biggest,
        recurring,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exp(desc: &str, cat: &str, cents: i64) -> MoneyEntry {
        MoneyEntry {
            id: desc.into(),
            kind: "expense".into(),
            date: "2026-08-01".into(),
            description: desc.into(),
            category: cat.into(),
            amount_cents: cents,
            year: 2026,
            month: 8,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn month_over_month_note() {
        let this_m = vec![exp("Dinner", "eating out", 12800)];
        let last_m = vec![MoneyEntry {
            month: 7,
            ..exp("Dinner", "eating out", 10000)
        }];
        let summary = summarize_month(2026, 8, &this_m);
        let prev = summarize_month(2026, 7, &last_m);
        let a = analyze_money_data(summary, Some(prev), &this_m, &last_m);
        assert!(a.notes.iter().any(|n| n.contains("eating out")));
    }

    #[test]
    fn pot_slug_strips_pot_word() {
        assert_eq!(pot_slug("Holiday pot"), "holiday");
        assert_eq!(pot_slug("the emergency"), "emergency");
    }
}
