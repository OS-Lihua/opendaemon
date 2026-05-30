use super::model::TaskStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTransition {
    Changed,
    Idempotent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStateError {
    InvalidTransition,
    AlreadyTerminal,
}

pub fn validate_transition(
    current: TaskStatus,
    next: TaskStatus,
) -> Result<TaskTransition, TaskStateError> {
    if current == next && current.is_terminal() {
        return Ok(TaskTransition::Idempotent);
    }
    if current.is_terminal() {
        return Err(TaskStateError::AlreadyTerminal);
    }
    let allowed = matches!(
        (current, next),
        (TaskStatus::Queued, TaskStatus::WaitingDirectoryLock)
            | (TaskStatus::Queued, TaskStatus::Cancelled)
            | (TaskStatus::WaitingDirectoryLock, TaskStatus::Preparing)
            | (TaskStatus::WaitingDirectoryLock, TaskStatus::Cancelled)
            | (TaskStatus::Preparing, TaskStatus::Running)
            | (TaskStatus::Preparing, TaskStatus::Failed)
            | (TaskStatus::Preparing, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::Completed)
            | (TaskStatus::Running, TaskStatus::Failed)
            | (TaskStatus::Running, TaskStatus::Cancelled)
            | (TaskStatus::Running, TaskStatus::TimedOut)
    );
    if allowed {
        Ok(TaskTransition::Changed)
    } else {
        Err(TaskStateError::InvalidTransition)
    }
}
