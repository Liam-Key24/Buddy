use std::ffi::OsString;
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
pub const DEFAULT_EXCLUSIONS: &[&str] = &[
    "Library",
    ".Trash",
    ".ssh",
    ".gnupg",
    ".cache",
    "Pictures",
    ".aws",
    ".kube",
    ".docker",
    ".config/gcloud",
];

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
    /// Existing components are canonicalized so symlink escapes cannot leave
    /// the root or enter an excluded tree.
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

        let lexical = resolve_lexical(&joined);

        if !lexical.starts_with(&self.root) {
            return Err(GuardError::OutsideRoot(lexical.display().to_string()));
        }

        let resolved = resolve_against_root(&lexical, &self.root)?;
        let root_cmp =
            existing_canonical(&self.root).unwrap_or_else(|| resolve_lexical(&self.root));

        if !resolved.starts_with(&root_cmp) && !resolved.starts_with(&self.root) {
            return Err(GuardError::OutsideRoot(resolved.display().to_string()));
        }

        for ex in &self.excluded {
            let ex_cmp = existing_canonical(ex).unwrap_or_else(|| resolve_lexical(ex));
            if resolved == ex_cmp || resolved.starts_with(&ex_cmp) {
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

fn existing_canonical(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        std::fs::canonicalize(path).ok()
    } else {
        None
    }
}

/// Canonicalize the deepest existing ancestor *at or under `root*`* and
/// re-append missing suffix components. Does not walk above `root`, so fake
/// test roots and non-existent homes stay lexical.
fn resolve_against_root(path: &Path, root: &Path) -> Result<PathBuf, GuardError> {
    let lexical = resolve_lexical(path);
    if !root.exists() {
        return Ok(lexical);
    }

    let mut current = lexical.clone();
    let mut suffix: Vec<OsString> = Vec::new();

    loop {
        if current.exists() {
            let mut canon = std::fs::canonicalize(&current).map_err(|e| {
                GuardError::Invalid(format!("cannot resolve {}: {e}", current.display()))
            })?;
            for part in suffix.into_iter().rev() {
                canon.push(part);
            }
            return Ok(canon);
        }
        if current == root {
            break;
        }
        match current.file_name() {
            Some(name) => {
                suffix.push(name.to_os_string());
                if !current.pop() {
                    break;
                }
            }
            None => break,
        }
        if !current.starts_with(root) {
            break;
        }
    }

    Ok(lexical)
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

    fn temp_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "buddy-pathguard-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
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
        assert!(matches!(
            g.check(".ssh/id_rsa"),
            Err(GuardError::Excluded(_))
        ));
    }

    #[test]
    fn allows_absolute_within_home() {
        let g = guard();
        assert_eq!(
            g.check("/home/user/notes.txt").unwrap(),
            PathBuf::from("/home/user/notes.txt")
        );
    }

    #[test]
    fn default_exclusions_include_credential_dirs() {
        for name in [".aws", ".kube", ".docker", ".config/gcloud"] {
            assert!(DEFAULT_EXCLUSIONS.contains(&name), "missing {name}");
        }
    }

    #[test]
    fn rejects_symlink_escape_outside_root() {
        let root = temp_root("escape");
        let outside = temp_root("outside");
        std::fs::write(outside.join("secret.txt"), "nope").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
            let g = PathGuard::new(root.clone(), vec![]);
            assert!(matches!(
                g.check("link/secret.txt"),
                Err(GuardError::OutsideRoot(_))
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn rejects_symlink_into_excluded() {
        let root = temp_root("excl");
        std::fs::create_dir_all(root.join(".ssh")).unwrap();
        std::fs::write(root.join(".ssh/id_rsa"), "key").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join(".ssh"), root.join("sneaky")).unwrap();
            let g = PathGuard::new(root.clone(), vec![".ssh".into()]);
            assert!(matches!(
                g.check("sneaky/id_rsa"),
                Err(GuardError::Excluded(_))
            ));
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn allows_real_file_inside_temp_root() {
        let root = temp_root("ok");
        std::fs::write(root.join("notes.txt"), "hi").unwrap();
        let g = PathGuard::new(root.clone(), vec![]);
        let got = g.check("notes.txt").unwrap();
        assert_eq!(got, std::fs::canonicalize(root.join("notes.txt")).unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }
}
