use opendaemon_console_ui::state::session::{StoredSession, storage_key};

#[test]
fn storage_key_is_stable() {
    assert_eq!(storage_key(), "opendaemon.console.session");
}

#[test]
fn stored_session_round_trips_json() {
    let session = StoredSession {
        base_url: "http://127.0.0.1:3000".to_owned(),
        credential_mode: "product".to_owned(),
        bearer_token: "secret".to_owned(),
        last_route: "/tasks".to_owned(),
        active_task_id: Some("task_1".to_owned()),
    };

    let encoded = serde_json::to_string(&session).unwrap();
    let decoded: StoredSession = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, session);
}
