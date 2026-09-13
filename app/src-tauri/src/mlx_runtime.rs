//! Explicit MLX process/generation state. Invalid transitions are rejected.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxState {
    #[default]
    Stopped,
    Starting,
    Ready,
    Generating,
    Cancelling,
    Failed,
}

impl MlxState {
    pub fn user_label(self) -> &'static str {
        match self {
            Self::Starting => "Starting local model",
            Self::Ready => "Ready",
            Self::Generating => "Working",
            Self::Cancelling => "Stopping safely",
            Self::Stopped => "Stopped",
            Self::Failed => "Service unavailable",
        }
    }

    pub fn can_transition(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Stopped, Self::Starting)
                | (Self::Starting, Self::Ready)
                | (Self::Starting, Self::Failed)
                | (Self::Ready, Self::Generating)
                | (Self::Generating, Self::Ready)
                | (Self::Generating, Self::Cancelling)
                | (Self::Generating, Self::Failed)
                | (Self::Cancelling, Self::Ready)
                | (Self::Cancelling, Self::Stopped)
                | (Self::Cancelling, Self::Failed)
                | (Self::Failed, Self::Starting)
                | (Self::Ready, Self::Stopped)
        )
    }

    pub fn transition(self, next: Self) -> Result<Self, String> {
        if self.can_transition(next) {
            Ok(next)
        } else {
            Err(format!("invalid mlx transition {self:?} → {next:?}"))
        }
    }

    pub fn accepts_new_generation(self) -> bool {
        matches!(self, Self::Ready | Self::Stopped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_transitions_match_spec() {
        let ok = [
            (MlxState::Stopped, MlxState::Starting),
            (MlxState::Starting, MlxState::Ready),
            (MlxState::Starting, MlxState::Failed),
            (MlxState::Ready, MlxState::Generating),
            (MlxState::Generating, MlxState::Ready),
            (MlxState::Generating, MlxState::Cancelling),
            (MlxState::Generating, MlxState::Failed),
            (MlxState::Cancelling, MlxState::Ready),
            (MlxState::Cancelling, MlxState::Stopped),
            (MlxState::Cancelling, MlxState::Failed),
            (MlxState::Failed, MlxState::Starting),
            (MlxState::Ready, MlxState::Stopped),
        ];
        for (from, to) in ok {
            assert_eq!(from.transition(to).unwrap(), to);
        }
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        assert!(MlxState::Stopped.transition(MlxState::Generating).is_err());
        assert!(MlxState::Ready.transition(MlxState::Starting).is_err());
        assert!(MlxState::Generating.transition(MlxState::Stopped).is_err());
        assert!(MlxState::Failed.transition(MlxState::Ready).is_err());
        assert!(MlxState::Cancelling.transition(MlxState::Generating).is_err());
    }

    #[test]
    fn user_labels_are_not_process_jargon() {
        assert_eq!(MlxState::Starting.user_label(), "Starting local model");
        assert_eq!(MlxState::Generating.user_label(), "Working");
        assert!(!MlxState::Failed.user_label().to_ascii_lowercase().contains("pid"));
        assert!(!MlxState::Failed.user_label().to_ascii_lowercase().contains("port"));
    }

    #[test]
    fn cancelled_or_generating_rejects_a_new_generation() {
        assert!(!MlxState::Generating.accepts_new_generation());
        assert!(!MlxState::Cancelling.accepts_new_generation());
        assert!(MlxState::Ready.accepts_new_generation());
        assert!(MlxState::Stopped.accepts_new_generation());
    }
}
