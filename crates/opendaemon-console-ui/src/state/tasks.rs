use opendaemon_console_api::{dto::TaskEventView, events::EventCursor};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TaskTranscript {
    pub events: Vec<TaskEventView>,
    cursor: EventCursor,
}

impl TaskTranscript {
    pub fn apply(&mut self, event: TaskEventView) {
        self.cursor.observe(&event);
        self.events.push(event);
        self.events.sort_by_key(|event| event.sequence);
    }

    #[must_use]
    pub fn latest_cursor(&self) -> Option<u64> {
        self.cursor.latest_sequence()
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TaskState {
    pub active_task_id: Option<String>,
    pub transcript: TaskTranscript,
}
