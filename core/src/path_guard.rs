use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::error::ToolError;

#[derive(Debug, Error)]
pub enum GuardError {
    #[error("could not resolve home directory")]
    NoHome,
    #[error("path is outside the allowed home directory: {0}")]
    OutsideRoot(String),
    #[error("path is within an excluded location: {0}")]
    Excluded(String),
    #[error("invalid path: {0}")]
    Invalid(String),
}

impl From<GuardError> for ToolError {
    fn from(err: GuardError) -> Self {
        ToolError::ExecutionFailed(err.to_string())
    }
}

/// Default excluded subpaths used when the `fs_excluded_paths` setting is
/// missing or unparseable. Shared by any tool/feature that resolves paths
/// against the user's home directory.
pub const DEFAULT_EXCLUSIONS: &[&str] =
    &["Library", ".Trash", ".ssh", ".gnupg", ".cache", "Pictures"];

/// Parses the `fs_excluded_paths` setting (a JSON array of strings), falling
/// back to [`DEFAULT_EXCLUSIONS`] when absent or invalid.
pub fn excluded_paths_from_setting(raw: Option<String>) -> Vec<String> {
    raw.and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_else(|| DEFAULT_EXCLUSIONS.iter().map(|s| s.to_string()).collect())
}

/// Restricts filesystem access to the user's home directory, minus a
/// user-configurable list of excluded subpaths (e.g. `Library`, `.ssh`).
///
/// Excluded entries may be given as bare names (`Library`) which are treated as
/// relative to the home root, or as absolute paths.
pub struct PathGuard {
    root: PathBuf,
    excluded: Vec<PathBuf>,
}

impl PathGuard {
    pub fn new(root: PathBuf, excluded: Vec<String>) -> Self {
        let excluded = excluded
            .into_iter()
            .map(|entry| {
                let p = PathBuf::from(&entry);
                if p.is_absolute() {
                    p
                } else {
                    root.join(p)
                }
            })
            .collect();
        Self { root, excluded }
    }

    /// Build a guard rooted at the user's home directory.
    pub fn home(excluded: Vec<String>) -> Result<Self, GuardError> {
        let root = dirs::home_dir().ok_or(GuardError::NoHome)?;
        Ok(Self::new(root, excluded))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Validate a requested path. Accepts absolute paths or paths relative to
    /// the home root. Returns the resolved absolute path when allowed.
    ///
    /// For paths that do not yet exist (e.g. new files) the parent chain is used
    /// for resolution so that create operations are still guarded.
    pub fn check(&self, requested: &str) -> Result<PathBuf, GuardError> {
        if requested.trim().is_empty() {
            return Err(GuardError::Invalid("empty path".into()));
        }

        let raw = PathBuf::from(requested);
        let joined = if raw.is_absolute() {
            raw
        } else {
            self.root.join(raw)
        };

        let resolved = resolve_lexical(&joined);

        if !resolved.starts_with(&self.root) {
            return Err(GuardError::OutsideRoot(resolved.display().to_string()));
        }

        for ex in &self.excluded {
            if resolved == *ex || resolved.starts_with(ex) {
                return Err(GuardError::Excluded(resolved.display().to_string()));
            }
        }

        Ok(resolved)
    }
}

/// Lexically normalize a path (resolve `.` and `..`) without touching the
/// filesystem, so it works for not-yet-existing files while still preventing
/// traversal escapes.
fn resolve_lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> PathGuard {
        PathGuard::new(
            PathBuf::from("/home/user"),
            vec!["Library".into(), ".ssh".into()],
        )
    }

    #[test]
    fn allows_relative_within_home() {
        let g = guard();
        assert_eq!(
            g.check("projects/app/main.rs").unwrap(),
            PathBuf::from("/home/user/projects/app/main.rs")
        );
    }

    #[test]
    fn rejects_traversal_escape() {
        let g = guard();
        assert!(matches!(
            g.check("../../etc/passwd"),
            Err(GuardError::OutsideRoot(_))
        ));
    }

    #[test]
    fn rejects_excluded_subtree() {
        let g = guard();
        assert!(matches!(
            g.check("Library/Keychains/x"),
            Err(GuardError::Excluded(_))
        ));
        assert!(matches!(g.check(".ssh/id_rsa"), Err(GuardError::Excluded(_))));
    }

    #[test]
    fn allows_absolute_within_home() {
        let g = guard();
        assert_eq!(
            g.check("/home/user/notes.txt").unwrap(),
            PathBuf::from("/home/user/notes.txt")
        );
    }
}
