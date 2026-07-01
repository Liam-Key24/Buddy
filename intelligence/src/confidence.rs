use buddy_memory::MemoryKind;

pub struct ConfidenceScorer;

impl ConfidenceScorer {
    pub fn initial_confidence(source: &str, inferred: bool) -> f64 {
        match source {
            "explicit" => 0.95,
            _ if inferred => 0.5,
            _ => 0.7,
        }
    }

    pub fn on_confirmation(current: f64) -> f64 {
        (current + 0.1).min(0.99)
    }

    pub fn on_contradiction(current: f64) -> f64 {
        current * 0.5
    }

    pub fn effective_importance(kind: MemoryKind, confidence: Option<f64>, base: f64) -> f64 {
        let conf = confidence.unwrap_or(base);
        match kind {
            MemoryKind::Working | MemoryKind::Handover => base.max(conf),
            _ => base * 0.5 + conf * 0.5,
        }
    }

    pub fn should_include(confidence: Option<f64>, similarity: f32) -> bool {
        match confidence {
            Some(c) if c < 0.3 => similarity >= 0.85,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_high_confidence() {
        assert!((ConfidenceScorer::initial_confidence("explicit", false) - 0.95).abs() < 1e-5);
    }

    #[test]
    fn confirmation_caps_at_99() {
        assert!((ConfidenceScorer::on_confirmation(0.95) - 0.99).abs() < 1e-5);
    }
}
