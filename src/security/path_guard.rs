use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathGuardError {
    InvalidDirectoryPath,
    PathNotDirectory,
    PathOutsideGrant,
    SymlinkEscape,
}

impl PathGuardError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidDirectoryPath => "invalid_directory_path",
            Self::PathNotDirectory => "path_not_directory",
            Self::PathOutsideGrant => "path_outside_grant",
            Self::SymlinkEscape => "symlink_escape",
        }
    }

    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::InvalidDirectoryPath => "invalid directory path",
            Self::PathNotDirectory => "path is not a directory",
            Self::PathOutsideGrant => "path is outside the directory grant",
            Self::SymlinkEscape => "path escapes the directory grant through a symlink",
        }
    }
}

pub fn canonicalize_grant_path(path: impl AsRef<Path>) -> Result<PathBuf, PathGuardError> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        return Err(PathGuardError::InvalidDirectoryPath);
    }

    let canonical = path
        .canonicalize()
        .map_err(|_| PathGuardError::InvalidDirectoryPath)?;
    if !canonical.is_dir() {
        return Err(PathGuardError::PathNotDirectory);
    }

    Ok(canonical)
}

pub fn ensure_child_path_within_grant(
    grant_root: impl AsRef<Path>,
    candidate_path: impl AsRef<Path>,
) -> Result<PathBuf, PathGuardError> {
    let grant_root = canonicalize_grant_path(grant_root)?;
    let candidate_path = candidate_path.as_ref();
    if candidate_path.as_os_str().is_empty() {
        return Err(PathGuardError::InvalidDirectoryPath);
    }

    let resolved_candidate = if candidate_path.is_absolute() {
        candidate_path.to_path_buf()
    } else {
        grant_root.join(candidate_path)
    };
    let canonical_candidate = resolved_candidate
        .canonicalize()
        .map_err(|_| PathGuardError::InvalidDirectoryPath)?;

    if canonical_candidate.starts_with(&grant_root) {
        return Ok(canonical_candidate);
    }

    if path_contains_symlink(&resolved_candidate) {
        return Err(PathGuardError::SymlinkEscape);
    }

    Err(PathGuardError::PathOutsideGrant)
}

fn path_contains_symlink(path: &Path) -> bool {
    let mut current = PathBuf::new();

    for component in path.components() {
        current.push(component);
        if std::fs::symlink_metadata(&current)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }

    false
}

impl FromStr for PathGuardError {
    type Err = ();

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        match code {
            "invalid_directory_path" => Ok(Self::InvalidDirectoryPath),
            "path_not_directory" => Ok(Self::PathNotDirectory),
            "path_outside_grant" => Ok(Self::PathOutsideGrant),
            "symlink_escape" => Ok(Self::SymlinkEscape),
            _ => Err(()),
        }
    }
}
