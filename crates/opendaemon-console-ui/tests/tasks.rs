use opendaemon_console_api::dto::TaskEventView;
use opendaemon_console_ui::state::tasks::TaskTranscript;

#[test]
fn transcript_appends_events_and_tracks_cursor() {
    let mut transcript = TaskTranscript::default();
    transcript.apply(event(1));
    transcript.apply(event(3));
    transcript.apply(event(2));

    assert_eq!(transcript.events.len(), 3);
    assert_eq!(transcript.latest_cursor(), Some(3));
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
