//! Domain intelligence packages. Retrieval is generic — adding a skill
//! must not add a central runtime branch.

/// Input to a skill detector. Keep this cheap: no model, no I/O.
#[derive(Debug, Clone, Copy)]
pub struct SkillDetectIn<'a> {
    pub text: &'a str,
    pub lower: &'a str,
    pub page: &'a str,
}

/// One domain package. Tools stay on plugins; skills only describe semantics
/// and which tool prefixes to attach.
#[derive(Debug, Clone, Copy)]
pub struct SkillSpec {
    pub id: &'static str,
    pub domain: &'static str,
    pub description: &'static str,
    pub tool_prefixes: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub presentation_hint: &'static str,
    /// Ambiguous day dumps attach every skill with this set.
    pub include_in_full_kit: bool,
    pub detect: fn(&SkillDetectIn<'_>) -> bool,
}

/// Return matching skills, or the full dump kit when nothing matched.
pub fn retrieve_skills<'a>(
    catalog: &'a [SkillSpec],
    input: &SkillDetectIn<'_>,
    dump_fallback: bool,
) -> Vec<&'a SkillSpec> {
    let hits: Vec<&'a SkillSpec> = catalog
        .iter()
        .filter(|skill| (skill.detect)(input))
        .collect();
    if hits.is_empty() && dump_fallback {
        catalog
            .iter()
            .filter(|skill| skill.include_in_full_kit)
            .collect()
    } else {
        hits
    }
}

pub fn prefixes_from_skills(skills: &[&SkillSpec]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for skill in skills {
        for prefix in skill.tool_prefixes {
            if !out.contains(prefix) {
                out.push(*prefix);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always(_input: &SkillDetectIn<'_>) -> bool {
        true
    }
    fn never(_input: &SkillDetectIn<'_>) -> bool {
        false
    }

    const A: SkillSpec = SkillSpec {
        id: "a",
        domain: "a",
        description: "a",
        tool_prefixes: &["a."],
        examples: &[],
        presentation_hint: "",
        include_in_full_kit: true,
        detect: always,
    };
    const B: SkillSpec = SkillSpec {
        id: "b",
        domain: "b",
        description: "b",
        tool_prefixes: &["b."],
        examples: &[],
        presentation_hint: "",
        include_in_full_kit: true,
        detect: never,
    };

    #[test]
    fn retrieve_returns_hits_without_dump_kit() {
        let input = SkillDetectIn {
            text: "x",
            lower: "x",
            page: "",
        };
        let hits = retrieve_skills(&[A, B], &input, true);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "a");
    }

    #[test]
    fn empty_hits_use_full_kit() {
        let input = SkillDetectIn {
            text: "x",
            lower: "x",
            page: "",
        };
        let hits = retrieve_skills(&[B], &input, true);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "b");
    }
}
