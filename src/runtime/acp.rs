use std::{collections::BTreeMap, process::Stdio, sync::Arc, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Mutex, oneshot},
    task::JoinHandle,
    time,
};

use crate::{
    registry::AcpTransport,
    task::{
        event::{PermissionDecision, PermissionRequestEvent, TaskEventType},
        permission::PermissionResponseRequest,
    },
};

use super::adapter::{
    RuntimeAdapter, RuntimeAdapterError, RuntimeCancelOutcome, RuntimeExecutionOutcome,
    RuntimeExecutionRequest, RuntimeOutputEvent,
};

const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct LocalAcpAdapter {
    default_timeout: Duration,
    cancellations: Arc<Mutex<BTreeMap<String, oneshot::Sender<()>>>>,
}

impl Default for LocalAcpAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_EXECUTION_TIMEOUT)
    }
}

impl LocalAcpAdapter {
    #[must_use]
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            default_timeout,
            cancellations: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    async fn execute_inner(&self, request: RuntimeExecutionRequest) -> RuntimeExecutionOutcome {
        let Some(acp) = &request.manifest.acp else {
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "acp_invalid_configuration",
                    "provider acp configuration is missing",
                ),
                Vec::new(),
            );
        };

        if acp.transport != AcpTransport::Stdio {
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "acp_invalid_configuration",
                    "only local stdio acp transport is implemented",
                ),
                Vec::new(),
            );
        }

        if !request.workspace.working_directory.is_dir() {
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "working_directory_missing",
                    "prepared working directory does not exist",
                ),
                Vec::new(),
            );
        }

        let Some(command) = acp.command.as_ref() else {
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "acp_invalid_configuration",
                    "acp stdio transport requires command metadata",
                ),
                Vec::new(),
            );
        };

        let timeout = if request.timeout_seconds == 0 {
            self.default_timeout
        } else {
            Duration::from_secs(request.timeout_seconds)
        };
        let mut child = Command::new(&request.executable);
        child
            .args(command.iter().skip(1))
            .current_dir(&request.workspace.working_directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();

        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.cancellations
            .lock()
            .await
            .insert(request.task_id.clone(), cancel_tx);

        let mut child = match child.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.cancellations.lock().await.remove(&request.task_id);
                return RuntimeExecutionOutcome::failed(
                    None,
                    RuntimeAdapterError::new(
                        "acp_session_start_failed",
                        format!("failed to spawn acp provider command: {error}"),
                    ),
                    Vec::new(),
                );
            }
        };

        let Some(mut stdin) = child.stdin.take() else {
            self.cancellations.lock().await.remove(&request.task_id);
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "acp_handshake_failed",
                    "acp provider stdin is unavailable",
                ),
                Vec::new(),
            );
        };
        let Some(stdout) = child.stdout.take() else {
            self.cancellations.lock().await.remove(&request.task_id);
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "acp_handshake_failed",
                    "acp provider stdout is unavailable",
                ),
                Vec::new(),
            );
        };
        let stderr = child.stderr.take();

        if stdin
            .write_all(request.task.prompt.as_bytes())
            .await
            .is_err()
            || stdin.write_all(b"\n").await.is_err()
        {
            self.cancellations.lock().await.remove(&request.task_id);
            let _ = child.kill().await;
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "acp_handshake_failed",
                    "failed to send task prompt to acp provider",
                ),
                Vec::new(),
            );
        }

        let stdout_handle = tokio::spawn(handle_stdout(
            request.task_id.clone(),
            request.provider_id.clone(),
            request.task_event_service.clone(),
            stdout,
            stdin,
        ));
        let stderr_handle = stderr.map(read_stderr);
        let wait = wait_for_child(&mut child, timeout, cancel_rx).await;
        self.cancellations.lock().await.remove(&request.task_id);

        let stdout_result = match stdout_handle.await {
            Ok(result) => result,
            Err(error) => {
                return RuntimeExecutionOutcome::failed(
                    None,
                    RuntimeAdapterError::new(
                        "acp_transport_closed",
                        format!("acp stdout task failed: {error}"),
                    ),
                    Vec::new(),
                );
            }
        };
        let stderr_events = match stderr_handle {
            Some(handle) => handle.await.unwrap_or_default(),
            None => Vec::new(),
        };
        let mut events = stdout_result.events;
        events.extend(stderr_events);

        if let Some(error) = stdout_result.error {
            return RuntimeExecutionOutcome::failed(None, error, events);
        }

        match wait {
            ChildWait::Exited(status) if status.success() => {
                let mut outcome = RuntimeExecutionOutcome::completed(status.code(), events);
                outcome.session_id = stdout_result.session_id;
                outcome.final_message = "acp session completed".to_owned();
                outcome
            }
            ChildWait::Exited(status) => RuntimeExecutionOutcome::failed(
                status.code(),
                RuntimeAdapterError::new(
                    "adapter_execution_failed",
                    format!("acp provider command exited with status {status}"),
                ),
                events,
            ),
            ChildWait::TimedOut => RuntimeExecutionOutcome::timed_out(events),
            ChildWait::Cancelled => RuntimeExecutionOutcome::cancelled(events),
            ChildWait::WaitFailed(error) => RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "adapter_execution_failed",
                    format!("failed to wait for acp provider command: {error}"),
                ),
                events,
            ),
        }
    }
}

impl RuntimeAdapter for LocalAcpAdapter {
    fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RuntimeExecutionOutcome> + Send + '_>>
    {
        Box::pin(self.execute_inner(request))
    }

    fn cancel(
        &self,
        task_id: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = RuntimeCancelOutcome> + Send + '_>>
    {
        let task_id = task_id.to_owned();
        Box::pin(async move {
            let sender = self.cancellations.lock().await.remove(&task_id);
            match sender {
                Some(sender) => RuntimeCancelOutcome {
                    cancelled: sender.send(()).is_ok(),
                    error: None,
                },
                None => RuntimeCancelOutcome {
                    cancelled: false,
                    error: Some(RuntimeAdapterError::new(
                        "command_cancelled",
                        "no running acp session for task",
                    )),
                },
            }
        })
    }
}

#[derive(Debug)]
struct StdoutResult {
    session_id: Option<String>,
    events: Vec<RuntimeOutputEvent>,
    error: Option<RuntimeAdapterError>,
}

async fn handle_stdout(
    task_id: String,
    provider_id: String,
    task_event_service: Option<crate::task::service::TaskEventService>,
    stdout: tokio::process::ChildStdout,
    mut stdin: tokio::process::ChildStdin,
) -> StdoutResult {
    let mut lines = BufReader::new(stdout).lines();
    let mut session_id = None;
    let mut events = Vec::new();

    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(frame) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(id) = frame.get("session_id").and_then(Value::as_str) {
            session_id = Some(id.to_owned());
        }

        if frame.get("type").and_then(Value::as_str) == Some("permission.requested") {
            let Some(service) = task_event_service.clone() else {
                return StdoutResult {
                    session_id,
                    events,
                    error: Some(RuntimeAdapterError::new(
                        "acp_permission_not_supported",
                        "acp permission requests require task event service integration",
                    )),
                };
            };
            match bridge_permission_request(&task_id, &provider_id, &service, &frame, &mut stdin)
                .await
            {
                Ok(permission_events) => events.extend(permission_events),
                Err(error) => {
                    return StdoutResult {
                        session_id,
                        events,
                        error: Some(error),
                    };
                }
            }
            continue;
        }

        if let Some(event) = normalize_acp_event(&frame) {
            events.push(event);
        }
    }

    StdoutResult {
        session_id,
        events,
        error: None,
    }
}

async fn bridge_permission_request(
    task_id: &str,
    provider_id: &str,
    task_event_service: &crate::task::service::TaskEventService,
    frame: &Value,
    stdin: &mut tokio::process::ChildStdin,
) -> Result<Vec<RuntimeOutputEvent>, RuntimeAdapterError> {
    let request_id = frame
        .get("request_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RuntimeAdapterError::new(
                "acp_permission_not_supported",
                "acp permission request is missing request_id",
            )
        })?;
    let summary = frame
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("permission request");
    let permission_kind = frame
        .get("permission_kind")
        .and_then(Value::as_str)
        .unwrap_or("provider_permission");
    let expires_at = frame
        .get("expires_at")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let request_event = PermissionRequestEvent {
        request_id: request_id.to_owned(),
        provider_id: provider_id.to_owned(),
        permission_kind: permission_kind.to_owned(),
        summary: summary.to_owned(),
        details: Some(frame.clone()),
        options: vec![PermissionDecision::Approve, PermissionDecision::Deny],
        expires_at,
    };
    task_event_service
        .store()
        .record_permission_request(task_id, request_event.clone())
        .map_err(|error| {
            RuntimeAdapterError::new(
                "acp_permission_not_supported",
                format!("failed to store acp permission request: {error:?}"),
            )
        })?;
    let decision = task_event_service
        .await_permission_decision(task_id, request_id)
        .await
        .map_err(|error| {
            RuntimeAdapterError::new(
                "acp_permission_not_supported",
                format!("failed to await acp permission decision: {error:?}"),
            )
        })?;
    let response = PermissionResponseRequest {
        request_id: request_id.to_owned(),
        decision: decision.decision,
        reason: decision.reason.clone(),
    };
    let provider_response = json!({
        "type": "permission.response",
        "request_id": response.request_id,
        "decision": match response.decision {
            PermissionDecision::Approve => "approve",
            PermissionDecision::Deny => "deny",
        },
        "reason": response.reason,
    });
    stdin
        .write_all(provider_response.to_string().as_bytes())
        .await
        .map_err(|error| {
            RuntimeAdapterError::new(
                "acp_transport_closed",
                format!("failed to send acp permission response: {error}"),
            )
        })?;
    stdin.write_all(b"\n").await.map_err(|error| {
        RuntimeAdapterError::new(
            "acp_transport_closed",
            format!("failed to flush acp permission response: {error}"),
        )
    })?;

    Ok(Vec::new())
}

fn read_stderr(stderr: tokio::process::ChildStderr) -> JoinHandle<Vec<RuntimeOutputEvent>> {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut events = Vec::new();

        while let Ok(Some(line)) = lines.next_line().await {
            events.push(RuntimeOutputEvent {
                kind: TaskEventType::ProcessStderr,
                payload: json!({"text": line, "stream": "stderr"}),
            });
        }

        events
    })
}

fn normalize_acp_event(frame: &Value) -> Option<RuntimeOutputEvent> {
    let event_type = frame.get("type")?.as_str()?;

    match event_type {
        "message.delta" => Some(RuntimeOutputEvent {
            kind: TaskEventType::ProcessStdout,
            payload: json!({
                "text": frame.get("text").and_then(Value::as_str).unwrap_or_default(),
                "stream": "stdout",
                "details": frame
            }),
        }),
        "permission.decided" => Some(RuntimeOutputEvent {
            kind: TaskEventType::ProviderPermissionDecided,
            payload: frame.clone(),
        }),
        _ => None,
    }
}

enum ChildWait {
    Exited(std::process::ExitStatus),
    TimedOut,
    Cancelled,
    WaitFailed(std::io::Error),
}

async fn wait_for_child(
    child: &mut tokio::process::Child,
    timeout: Duration,
    mut cancel_rx: oneshot::Receiver<()>,
) -> ChildWait {
    tokio::select! {
        status = child.wait() => match status {
            Ok(status) => ChildWait::Exited(status),
            Err(error) => ChildWait::WaitFailed(error),
        },
        () = time::sleep(timeout) => {
            let _ = child.kill().await;
            ChildWait::TimedOut
        }
        _ = &mut cancel_rx => {
            let _ = child.kill().await;
            ChildWait::Cancelled
        }
    }
}
