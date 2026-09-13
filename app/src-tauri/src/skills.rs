//! Buddy skill catalog. Domain matching lives here so `native_loop` only
//! retrieves prefixes — adding a domain does not add a controller branch.

use buddy_calendar::parse_when_label;
use buddy_core::{prefixes_from_skills, retrieve_skills, SkillDetectIn, SkillSpec};

use crate::native_loop::{
    extract_doc_title, has_actiony, has_any, has_look_cue, has_word, looks_like_clock_pin,
    looks_like_code_request, looks_like_doc_look, looks_like_dump, looks_like_organize,
    looks_like_spark_list, looks_like_spark_save, wants_doc_format,
};

fn detect_organise(input: &SkillDetectIn<'_>) -> bool {
    parse_when_label(input.lower).is_some()
        || has_look_cue(input.lower)
        || looks_like_organize(input.lower)
        || looks_like_clock_pin(input.lower)
        || has_any(
            input.lower,
            &[
                "find a slot",
                "a slot for",
                "free slot",
                "find time for",
                "find me a slot",
            ],
        )
}

fn detect_todos(input: &SkillDetectIn<'_>) -> bool {
    has_any(
        input.lower,
        &[
            "todo",
            "to-do",
            "to do",
            "deadline",
            "get done",
            "need to do",
            "my tasks",
        ],
    ) || (input.page.contains("page: todo") && has_actiony(input.lower))
}

fn detect_study(input: &SkillDetectIn<'_>) -> bool {
    has_any(
        input.lower,
        &[
            "exam",
            "assignment",
            "assignments",
            "flashcard",
            "revision",
            "studied",
            "topic",
            "topics",
        ],
    ) || (has_word(input.lower, "study")
        && has_any(
            input.lower,
            &[
                "session", "plan", "log", "schedule", "add", "hours", "what", "event", "subject",
            ],
        ))
        || (input.page.contains("page: study") && has_actiony(input.lower))
}

fn detect_fitness(input: &SkillDetectIn<'_>) -> bool {
    has_any(
        input.lower,
        &[
            "calorie",
            "fridge",
            "workout",
            "workouts",
            "i ate",
            "i eat",
            "climbed",
            "climbing",
            "climb",
            "bouldering",
            "weigh in",
            "log weight",
            "what did i eat",
            "food log",
        ],
    ) || has_word(input.lower, "weight")
        || has_word(input.lower, "weigh")
        || has_word(input.lower, "protein")
        || (has_word(input.lower, "eat")
            && has_any(
                input.lower,
                &["tonight", "today", "log", "should i", "what should"],
            ))
        || (input.page.contains("page: fitness") && has_actiony(input.lower))
}

fn detect_money(input: &SkillDetectIn<'_>) -> bool {
    has_any(
        input.lower,
        &[
            "spent",
            "earned",
            "invoice",
            "expense",
            "transactions",
            "what did i spend",
            "pots",
            "savings pot",
        ],
    ) || has_word(input.lower, "spend")
        || has_word(input.lower, "pot")
        || has_word(input.lower, "paid")
        || input.lower.contains('£')
        || (has_word(input.lower, "money") && has_actiony(input.lower))
        || (input.page.contains("page: money") && has_actiony(input.lower))
}

fn detect_docs(input: &SkillDetectIn<'_>) -> bool {
    wants_doc_format(input.text)
        || extract_doc_title(input.text).is_some()
        || looks_like_doc_look(input.lower)
        || (has_any(input.lower, &["document", "documents", "docs", "sheet"])
            && has_actiony(input.lower))
        || (input.page.contains("page: documents") && has_actiony(input.lower))
}

fn detect_socials(input: &SkillDetectIn<'_>) -> bool {
    has_any(input.lower, &["linkedin", "twitter", "weekly review", "socials"])
        || (has_word(input.lower, "posting") && has_word(input.lower, "social"))
        || (input.page.contains("page: socials") && has_actiony(input.lower))
}

fn detect_research(input: &SkillDetectIn<'_>) -> bool {
    (has_word(input.lower, "research")
        && (has_actiony(input.lower) || has_any(input.lower, &["what", "findings", "session", "sources"])))
        || (input.page.contains("page: research") && has_actiony(input.lower))
}

fn detect_build(input: &SkillDetectIn<'_>) -> bool {
    looks_like_code_request(input.lower)
}

fn detect_sparks(input: &SkillDetectIn<'_>) -> bool {
    looks_like_spark_save(input.lower)
        || has_word(input.lower, "spark")
        || has_word(input.lower, "sparks")
        || looks_like_spark_list(input.lower)
        || input.lower.starts_with("idea:")
        || input.lower.contains("idea: ")
}

fn detect_life_admin(input: &SkillDetectIn<'_>) -> bool {
    has_any(input.lower, &["send email", "email this", "git push", "push to github"])
}

fn detect_goals(input: &SkillDetectIn<'_>) -> bool {
    let record_only = has_any(input.lower, &["i spent", "i ate", "i earned"])
        && !has_any(input.lower, &["i want", "below", "goal"]);
    if record_only {
        return false;
    }
    has_any(
        input.lower,
        &[
            "i want to",
            "my goal",
            "my goals",
            "by december",
            "keep food spending",
            "plan my goals",
            "behind on",
        ],
    ) || (has_word(input.lower, "goal") && has_actiony(input.lower))
}

pub const SKILLS: &[SkillSpec] = &[
    SkillSpec {
        id: "organise",
        domain: "calendar",
        description: "Look, pin, and organise time",
        tool_prefixes: &["calendar.", "lifestyle.", "dream.", "work."],
        examples: &["what's on Friday", "dentist tomorrow at 2pm"],
        presentation_hint: "Propose calendar writes before commit",
        include_in_full_kit: true,
        detect: detect_organise,
    },
    SkillSpec {
        id: "todos",
        domain: "todo",
        description: "Capture and list tasks",
        tool_prefixes: &["todo."],
        examples: &["add a todo to buy milk"],
        presentation_hint: "Todos stay todos — they are not goals",
        include_in_full_kit: true,
        detect: detect_todos,
    },
    SkillSpec {
        id: "study",
        domain: "study",
        description: "Subjects, sessions, assignments",
        tool_prefixes: &["study."],
        examples: &["log a 60 minute study session"],
        presentation_hint: "Record study; schedule only when asked",
        include_in_full_kit: true,
        detect: detect_study,
    },
    SkillSpec {
        id: "fitness",
        domain: "fitness",
        description: "Food, workouts, climbing, weight",
        tool_prefixes: &["fitness."],
        examples: &["I ate a burrito", "list my logged workouts"],
        presentation_hint: "Record first; do not invent a goal",
        include_in_full_kit: true,
        detect: detect_fitness,
    },
    SkillSpec {
        id: "money",
        domain: "money",
        description: "Spend, income, pots",
        tool_prefixes: &["money."],
        examples: &["I spent £20 on dinner"],
        presentation_hint: "Store the record unless a goal is explicit",
        include_in_full_kit: true,
        detect: detect_money,
    },
    SkillSpec {
        id: "docs",
        domain: "docs",
        description: "In-app documents",
        tool_prefixes: &["docs."],
        examples: &["open bello.today"],
        presentation_hint: "Edit documents in place",
        include_in_full_kit: true,
        detect: detect_docs,
    },
    SkillSpec {
        id: "socials",
        domain: "socials",
        description: "Weekly social planning",
        tool_prefixes: &["socials."],
        examples: &["show this week's social plan"],
        presentation_hint: "Approve posts before pinning",
        include_in_full_kit: true,
        detect: detect_socials,
    },
    SkillSpec {
        id: "research",
        domain: "research",
        description: "Research sessions",
        tool_prefixes: &["research."],
        examples: &["what did I find in research"],
        presentation_hint: "Keep findings structured",
        include_in_full_kit: true,
        detect: detect_research,
    },
    SkillSpec {
        id: "sparks",
        domain: "spark",
        description: "Capture ideas without committing them",
        tool_prefixes: &["save_spark", "update_spark", "list_sparks"],
        examples: &["Idea: build a small climbing progress app"],
        presentation_hint: "Capture quickly; promote later",
        include_in_full_kit: true,
        detect: detect_sparks,
    },
    SkillSpec {
        id: "build",
        domain: "code",
        description: "Project and local file edits",
        tool_prefixes: &["coder.", "read_file", "write_file", "edit_file", "delete_file", "list_dir"],
        examples: &["help me code a parser in this project"],
        presentation_hint: "Stay in the trusted runtime",
        include_in_full_kit: false,
        detect: detect_build,
    },
    SkillSpec {
        id: "goals",
        domain: "goal",
        description: "Turn ambiguous goals into an approved calendar plan",
        tool_prefixes: &["goal."],
        examples: &[
            "By December I want to climb V6, save £2,000 and read two books.",
            "I want to keep food spending below £250 this month.",
        ],
        presentation_hint: "Ask only plan-changing questions; never commit the week silently",
        include_in_full_kit: false,
        detect: detect_goals,
    },
    SkillSpec {
        id: "life_admin",
        domain: "external",
        description: "Email and git push",
        tool_prefixes: &["send_email", "git_push"],
        examples: &["send email to confirm the booking"],
        presentation_hint: "External actions need permission",
        include_in_full_kit: false,
        detect: detect_life_admin,
    },
];

/// Prefixes for the complete loop. Dump fallback attaches the full life kit.
pub fn prefixes_for_turn(text: &str, ui_context: Option<&str>) -> Vec<&'static str> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return Vec::new();
    }
    let page = ui_context.unwrap_or("").to_ascii_lowercase();
    let input = SkillDetectIn {
        text,
        lower: &lower,
        page: &page,
    };
    let skills = retrieve_skills(SKILLS, &input, false);
    if !skills.is_empty() {
        return prefixes_from_skills(&skills);
    }
    if looks_like_dump(text) {
        let kit: Vec<&SkillSpec> = SKILLS.iter().filter(|s| s.include_in_full_kit).collect();
        return prefixes_from_skills(&kit);
    }
    Vec::new()
}

pub fn skill_ids_for_turn(text: &str, ui_context: Option<&str>) -> Vec<&'static str> {
    let lower = text.trim().to_ascii_lowercase();
    let page = ui_context.unwrap_or("").to_ascii_lowercase();
    let input = SkillDetectIn {
        text,
        lower: &lower,
        page: &page,
    };
    retrieve_skills(SKILLS, &input, looks_like_dump(text))
        .into_iter()
        .map(|s| s.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holdouts_retrieve_expected_skills() {
        let cases = [
            ("I ate a burrito", "fitness"),
            ("I spent £20 on dinner", "money"),
            ("Idea: build a small climbing progress app", "sparks"),
            ("list my logged workouts", "fitness"),
            ("what's on Friday", "organise"),
            ("add a todo to buy milk", "todos"),
            (
                "By December I want to climb V6, save £2,000 and read two books.",
                "goals",
            ),
        ];
        for (text, skill) in cases {
            let ids = skill_ids_for_turn(text, None);
            assert!(
                ids.contains(&skill),
                "{text:?} expected {skill}, got {ids:?}"
            );
        }
    }

    #[test]
    fn dump_fallback_attaches_life_kit_not_coder() {
        let text = "Tomorrow dentist at 10, buy milk, spent 25 on dinner, and an idea for a tracker";
        let prefixes = prefixes_for_turn(text, None);
        assert!(prefixes.iter().any(|p| *p == "calendar." || *p == "todo." || *p == "money."));
        assert!(!prefixes.iter().any(|p| *p == "coder."));
    }

    #[test]
    fn adding_a_skill_does_not_require_a_native_loop_branch() {
        assert!(SKILLS.iter().any(|s| s.id == "organise"));
        assert_eq!(SKILLS.iter().filter(|s| s.include_in_full_kit).count(), 9);
    }
}
