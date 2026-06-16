use crate::{dto::TaskEventView, error::ConsoleApiError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCursor {
    latest_sequence: Option<u64>,
}

impl EventCursor {
    #[must_use]
    pub fn latest_sequence(&self) -> Option<u64> {
        self.latest_sequence
    }

    pub fn observe(&mut self, event: &TaskEventView) {
        self.latest_sequence = Some(
            self.latest_sequence
                .map_or(event.sequence, |current| current.max(event.sequence)),
        );
    }
}

pub fn apply_sse_event(events: &mut Vec<TaskEventView>, next: TaskEventView) {
    events.retain(|event| event.sequence != next.sequence);
    events.push(next);
    events.sort_by_key(|event| event.sequence);
}

pub fn parse_sse_block(block: &str) -> Result<Option<TaskEventView>, ConsoleApiError> {
    let mut data = Vec::new();
    for line in block.lines() {
        if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data.push(rest.trim_start());
        }
    }

    if data.is_empty() {
        return Ok(None);
    }

    serde_json::from_str(&data.join("\n"))
        .map(Some)
        .map_err(|error| ConsoleApiError::Decode(error.to_string()))
}
