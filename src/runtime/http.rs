use std::{collections::BTreeMap, future::Future, pin::Pin};

use serde_json::{Value, json};
use tokio::sync::{Mutex, oneshot};

use crate::{
    registry::{HttpAuthScheme, HttpUploadMode},
    task::event::TaskEventType,
};

use super::adapter::{
    RuntimeAdapter, RuntimeAdapterError, RuntimeCancelOutcome, RuntimeExecutionOutcome,
    RuntimeExecutionRequest, RuntimeOutputEvent,
};

#[derive(Debug, Clone, Default)]
pub struct RemoteHttpAdapter {
    cancellations: std::sync::Arc<Mutex<BTreeMap<String, oneshot::Sender<()>>>>,
    client: reqwest::Client,
}

impl RemoteHttpAdapter {
    async fn execute_inner(&self, request: RuntimeExecutionRequest) -> RuntimeExecutionOutcome {
        let Some(http) = request.manifest.http.as_ref() else {
            return RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "http_invalid_configuration",
                    "provider http configuration is missing",
                ),
                Vec::new(),
            );
        };

        let package = match build_workspace_package(&request) {
            Ok(package) => package,
            Err(error) => return RuntimeExecutionOutcome::failed(None, error, Vec::new()),
        };

        let payload = json!({
            "task": {
                "id": request.task.id,
                "prompt": request.task.prompt,
                "model": request.agent_profile.model,
            },
            "runtime": {
                "id": request.runtime_id,
                "provider_id": request.provider_id,
            },
            "workspace": package,
            "opendaemon_auth": Value::Null,
        });

        let mut builder = self.client.post(&http.endpoint).json(&payload);
        if http.auth_scheme == HttpAuthScheme::Bearer {
            builder = builder.bearer_auth("");
        }

        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        self.cancellations
            .lock()
            .await
            .insert(request.task_id.clone(), cancel_tx);

        let response = tokio::select! {
            biased;
            _ = &mut cancel_rx => {
                self.cancellations.lock().await.remove(&request.task_id);
                return RuntimeExecutionOutcome::cancelled(Vec::new());
            }
            result = builder.send() => result
        };
        self.cancellations.lock().await.remove(&request.task_id);

        let response = match response {
            Ok(response) => response,
            Err(error) => {
                return RuntimeExecutionOutcome::failed(
                    None,
                    RuntimeAdapterError::new(
                        "http_request_failed",
                        format!("http provider request failed: {error}"),
                    ),
                    Vec::new(),
                );
            }
        };

        let response = match response.error_for_status() {
            Ok(response) => response,
            Err(error) => {
                return RuntimeExecutionOutcome::failed(
                    None,
                    RuntimeAdapterError::new(
                        "http_request_failed",
                        format!("http provider response failed: {error}"),
                    ),
                    Vec::new(),
                );
            }
        };

        let provider_result = match response.json::<Value>().await {
            Ok(value) => value,
            Err(error) => {
                return RuntimeExecutionOutcome::failed(
                    None,
                    RuntimeAdapterError::new(
                        "http_protocol_error",
                        format!("invalid http provider response: {error}"),
                    ),
                    Vec::new(),
                );
            }
        };

        let mut outcome = RuntimeExecutionOutcome::completed(
            Some(0),
            vec![RuntimeOutputEvent {
                kind: TaskEventType::ProcessStdout,
                payload: json!({ "text": "remote provider completed" }),
            }],
        );
        outcome.final_message = provider_result["final_message"]
            .as_str()
            .unwrap_or("remote provider completed")
            .to_owned();
        outcome.provider_result = Some(json!({
            "provider_response": provider_result,
            "remote_upload": {
                "provider_id": request.provider_id,
                "runtime_id": request.runtime_id,
                "endpoint_origin": http.endpoint,
                "upload_mode": match http.upload_mode {
                    HttpUploadMode::WorkspaceSubset => "workspace_subset",
                    HttpUploadMode::Diff => "diff",
                    HttpUploadMode::ContextOnly => "context_only",
                },
                "file_count": package["files"].as_array().map_or(0, |files| files.len()),
                "byte_count": package["byte_count"],
            }
        }));
        outcome
    }
}

impl RuntimeAdapter for RemoteHttpAdapter {
    fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> Pin<Box<dyn Future<Output = RuntimeExecutionOutcome> + Send + '_>> {
        Box::pin(self.execute_inner(request))
    }

    fn cancel(
        &self,
        task_id: &str,
    ) -> Pin<Box<dyn Future<Output = RuntimeCancelOutcome> + Send + '_>> {
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
                        "http_cancel_not_supported",
                        "no running remote http task for cancellation",
                    )),
                },
            }
        })
    }
}

fn build_workspace_package(
    request: &RuntimeExecutionRequest,
) -> Result<Value, RuntimeAdapterError> {
    let mut files = Vec::new();
    let mut byte_count = 0usize;
    let root = &request.workspace.working_directory;
    let entries = std::fs::read_dir(root).map_err(|error| {
        RuntimeAdapterError::new(
            "http_request_failed",
            format!("failed to read workspace for upload: {error}"),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            RuntimeAdapterError::new(
                "http_request_failed",
                format!("failed to inspect workspace entry: {error}"),
            )
        })?;
        let file_type = entry.file_type().map_err(|error| {
            RuntimeAdapterError::new(
                "http_request_failed",
                format!("failed to inspect workspace file type: {error}"),
            )
        })?;
        if !file_type.is_file() {
            continue;
        }

        let path = entry.path();
        let contents = std::fs::read_to_string(&path).map_err(|error| {
            RuntimeAdapterError::new(
                "http_request_failed",
                format!("failed to read workspace file: {error}"),
            )
        })?;
        byte_count += contents.len();
        files.push(json!({
            "path": path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            "content": contents,
        }));
    }

    Ok(json!({
        "source_directory_id": request.workspace.source_directory_id,
        "upload_mode": "workspace_subset",
        "files": files,
        "byte_count": byte_count,
    }))
}
