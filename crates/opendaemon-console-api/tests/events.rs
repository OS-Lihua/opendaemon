use opendaemon_console_api::{
    dto::TaskEventView,
    events::{EventCursor, apply_sse_event, parse_sse_block},
};

#[test]
fn parses_data_only_sse_block() {
    let event = parse_sse_block(
        r#"event: task_event
data: {"task_id":"task_1","sequence":7,"type":"task.started","payload":{},"created_at":"2026-06-16T00:00:00Z"}
"#,
    )
    .unwrap()
    .unwrap();

    assert_eq!(event.task_id, "task_1");
    assert_eq!(event.sequence, 7);
    assert_eq!(event.r#type, "task.started");
}

#[test]
fn ignores_comment_heartbeat_blocks() {
    assert!(parse_sse_block(": heartbeat\n").unwrap().is_none());
}

#[test]
fn cursor_tracks_highest_sequence() {
    let mut cursor = EventCursor::default();
    cursor.observe(&event(3));
    cursor.observe(&event(2));

    assert_eq!(cursor.latest_sequence(), Some(3));
}

#[test]
fn apply_sse_event_replaces_duplicate_and_sorts() {
    let mut events = vec![event(2), event(1)];
    apply_sse_event(&mut events, event(2));

    assert_eq!(
        events
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

fn event(sequence: u64) -> TaskEventView {
    TaskEventView {
        task_id: "task_1".to_owned(),
        sequence,
        r#type: "task.output".to_owned(),
        payload: serde_json::json!({ "text": format!("line {sequence}") }),
        created_at: "2026-06-16T00:00:00Z".to_owned(),
    }
}
