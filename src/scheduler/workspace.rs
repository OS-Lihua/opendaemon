use std::path::PathBuf;

use crate::security::directory::{DirectoryGrant, WorkspaceMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedWorkspace {
    pub workspace_mode: WorkspaceMode,
    pub working_directory: PathBuf,
    pub source_directory_id: String,
    pub worktree_path: Option<PathBuf>,
    pub branch_name: Option<String>,
}

#[derive(Debug)]
pub enum WorkspaceError {
    PrepareFailed,
}

pub trait WorkspacePreparer {
    fn prepare(
        &self,
        task_id: &str,
        grant: &DirectoryGrant,
        workspace_mode: WorkspaceMode,
    ) -> Result<PreparedWorkspace, WorkspaceError>;
}

#[derive(Debug, Clone, Copy)]
pub struct FailingWorkspacePreparer;

impl WorkspacePreparer for FailingWorkspacePreparer {
    fn prepare(
        &self,
        _task_id: &str,
        _grant: &DirectoryGrant,
        _workspace_mode: WorkspaceMode,
    ) -> Result<PreparedWorkspace, WorkspaceError> {
        Err(WorkspaceError::PrepareFailed)
    }
}

#[derive(Debug, Clone)]
pub struct FakeWorkspacePreparer {
    root: PathBuf,
}

impl FakeWorkspacePreparer {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl WorkspacePreparer for FakeWorkspacePreparer {
    fn prepare(
        &self,
        task_id: &str,
        grant: &DirectoryGrant,
        workspace_mode: WorkspaceMode,
    ) -> Result<PreparedWorkspace, WorkspaceError> {
        let source_directory_id = grant.id.clone();
        match workspace_mode {
            WorkspaceMode::Direct => Ok(PreparedWorkspace {
                workspace_mode,
                working_directory: PathBuf::from(&grant.path),
                source_directory_id,
                worktree_path: None,
                branch_name: None,
            }),
            WorkspaceMode::Worktree => {
                let worktree_path = self.root.join(task_id);
                Ok(PreparedWorkspace {
                    workspace_mode,
                    working_directory: worktree_path.clone(),
                    source_directory_id,
                    worktree_path: Some(worktree_path),
                    branch_name: Some(format!("opendaemon/{task_id}")),
                })
            }
        }
    }
}
