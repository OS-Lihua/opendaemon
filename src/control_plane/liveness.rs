use std::time::Duration;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::model::DaemonConnectionStatus;

#[derive(Debug, Clone, Copy)]
pub struct LivenessTracker {
    staleness_threshold: Duration,
}

impl LivenessTracker {
    #[must_use]
    pub const fn new(staleness_threshold: Duration) -> Self {
        Self {
            staleness_threshold,
        }
    }

    #[must_use]
    pub fn evaluate(
        &self,
        last_heartbeat_at: Option<&str>,
        now_rfc3339: &str,
        current: DaemonConnectionStatus,
    ) -> DaemonConnectionStatus {
        let Some(last_heartbeat_at) = last_heartbeat_at else {
            return DaemonConnectionStatus::Offline;
        };
        let Ok(last) = OffsetDateTime::parse(last_heartbeat_at, &Rfc3339) else {
            return DaemonConnectionStatus::Offline;
        };
        let Ok(now) = OffsetDateTime::parse(now_rfc3339, &Rfc3339) else {
            return current;
        };
        let elapsed = now - last;
        if elapsed.is_negative()
            || elapsed.whole_seconds() < self.staleness_threshold.as_secs() as i64
        {
            current
        } else {
            DaemonConnectionStatus::Offline
        }
    }
}
