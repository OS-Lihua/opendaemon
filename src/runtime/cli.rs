use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::{Mutex, oneshot},
    task::JoinHandle,
    time,
};

use crate::{
    registry::{CancelSignal, ExecutionInputMode},
    task::event::TaskEventType,
};

use super::{
    adapter::{
        RuntimeAdapter, RuntimeAdapterError, RuntimeCancelOutcome, RuntimeExecutionOutcome,
        RuntimeExecutionRequest, RuntimeOutputEvent,
    },
    template::{CommandTemplate, TemplateValues},
};

const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_EVENT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct LocalCliAdapter {
    default_timeout: Duration,
    cancellations: Arc<Mutex<BTreeMap<String, oneshot::Sender<()>>>>,
}

impl Default for LocalCliAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_EXECUTION_TIMEOUT)
    }
}

impl LocalCliAdapter {
    #[must_use]
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            default_timeout,
            cancellations: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    async fn execute_inner(&self, request: RuntimeExecutionRequest) -> RuntimeExecutionOutcome {
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

        let mut temp_prompt_file = None;
        let prompt_value = match request.manifest.execution.input_mode {
            ExecutionInputMode::Arg | ExecutionInputMode::Stdin => request.task.prompt.clone(),
            ExecutionInputMode::TempFile => {
                match write_prompt_file(&request.task_id, &request.task.prompt) {
                    Ok(path) => {
                        let value = path.to_string_lossy().into_owned();
                        temp_prompt_file = Some(path);
                        value
                    }
                    Err(error) => {
                        return RuntimeExecutionOutcome::failed(
                            None,
                            RuntimeAdapterError::new(
                                "adapter_execution_failed",
                                format!("failed to write prompt file: {error}"),
                            ),
                            Vec::new(),
                        );
                    }
                }
            }
        };

        let values = TemplateValues {
            prompt: Some(prompt_value),
            model: Some(request.agent_profile.model.clone()),
            workspace: Some(
                request
                    .workspace
                    .working_directory
                    .to_string_lossy()
                    .into_owned(),
            ),
            task_id: Some(request.task_id.clone()),
            agent_id: Some(request.agent_profile.id.clone()),
            directory_id: Some(request.directory_grant.id.clone()),
        };
        let mut args = match CommandTemplate::render_args(&request.manifest.execution.args, &values)
        {
            Ok(args) => args,
            Err(error) => {
                cleanup_temp_file(temp_prompt_file.as_deref());
                return RuntimeExecutionOutcome::failed(None, error, Vec::new());
            }
        };
        args.extend(request.agent_profile.provider_config.custom_args.clone());

        let timeout = if request.timeout_seconds == 0 {
            self.default_timeout
        } else {
            Duration::from_secs(request.timeout_seconds)
        };
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.cancellations
            .lock()
            .await
            .insert(request.task_id.clone(), cancel_tx);

        let mut command = Command::new(&request.executable);
        command
            .args(&args)
            .current_dir(&request.workspace.working_directory)
            .stdin(
                if request.manifest.execution.input_mode == ExecutionInputMode::Stdin {
                    Stdio::piped()
                } else {
                    Stdio::null()
                },
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();
        apply_minimal_environment(&mut command, &request);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.cancellations.lock().await.remove(&request.task_id);
                cleanup_temp_file(temp_prompt_file.as_deref());
                return RuntimeExecutionOutcome::failed(
                    None,
                    RuntimeAdapterError::new(
                        "command_spawn_failed",
                        format!("failed to spawn provider command: {error}"),
                    ),
                    Vec::new(),
                );
            }
        };

        if request.manifest.execution.input_mode == ExecutionInputMode::Stdin
            && let Some(mut stdin) = child.stdin.take()
        {
            let prompt = request.task.prompt.clone();
            tokio::spawn(async move {
                let _ = stdin.write_all(prompt.as_bytes()).await;
                let _ = stdin.shutdown().await;
            });
        }

        let stdout = child.stdout.take().map(read_pipe);
        let stderr = child.stderr.take().map(read_pipe);
        let wait = wait_for_child(
            child,
            timeout,
            cancel_rx,
            request.manifest.execution.cancel_signal,
        )
        .await;
        self.cancellations.lock().await.remove(&request.task_id);
        cleanup_temp_file(temp_prompt_file.as_deref());
        let events = output_events(await_pipe(stdout).await, await_pipe(stderr).await);

        match wait {
            ChildWait::Exited(status) if status.success() => {
                RuntimeExecutionOutcome::completed(status.code(), events)
            }
            ChildWait::Exited(status) => RuntimeExecutionOutcome::failed(
                status.code(),
                RuntimeAdapterError::new(
                    "adapter_execution_failed",
                    format!("provider command exited with status {status}"),
                ),
                events,
            ),
            ChildWait::TimedOut => RuntimeExecutionOutcome::timed_out(events),
            ChildWait::Cancelled => RuntimeExecutionOutcome::cancelled(events),
            ChildWait::WaitFailed(error) => RuntimeExecutionOutcome::failed(
                None,
                RuntimeAdapterError::new(
                    "adapter_execution_failed",
                    format!("failed to wait for provider command: {error}"),
                ),
                events,
            ),
        }
    }
}

impl RuntimeAdapter for LocalCliAdapter {
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
                        "no running provider command for task",
                    )),
                },
            }
        })
    }
}

enum ChildWait {
    Exited(std::process::ExitStatus),
    TimedOut,
    Cancelled,
    WaitFailed(std::io::Error),
}

async fn wait_for_child(
    mut child: tokio::process::Child,
    timeout: Duration,
    mut cancel_rx: oneshot::Receiver<()>,
    cancel_signal: CancelSignal,
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
            terminate_child(&mut child, cancel_signal).await;
            ChildWait::Cancelled
        }
    }
}

async fn terminate_child(child: &mut tokio::process::Child, cancel_signal: CancelSignal) {
    match cancel_signal {
        CancelSignal::Sigterm | CancelSignal::Sigint => {
            if send_unix_signal(child.id(), cancel_signal).is_ok() {
                let grace = time::sleep(Duration::from_millis(500));
                tokio::pin!(grace);
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => return,
                        Ok(None) => {}
                        Err(_) => break,
                    }
                    tokio::select! {
                        () = &mut grace => break,
                        () = time::sleep(Duration::from_millis(20)) => {}
                    }
                }
            }
            let _ = child.kill().await;
        }
        CancelSignal::Kill | CancelSignal::None => {
            let _ = child.kill().await;
        }
    }
}

#[cfg(unix)]
fn send_unix_signal(pid: Option<u32>, cancel_signal: CancelSignal) -> std::io::Result<()> {
    let Some(pid) = pid else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "missing child process id",
        ));
    };
    let signal = match cancel_signal {
        CancelSignal::Sigterm => "-TERM",
        CancelSignal::Sigint => "-INT",
        CancelSignal::Kill | CancelSignal::None => return Ok(()),
    };
    let status = std::process::Command::new("/bin/kill")
        .arg(signal)
        .arg(pid.to_string())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("failed to signal child process"))
    }
}

#[cfg(not(unix))]
fn send_unix_signal(_pid: Option<u32>, _cancel_signal: CancelSignal) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "Unix signals are not available on this platform",
    ))
}

fn apply_minimal_environment(command: &mut Command, request: &RuntimeExecutionRequest) {
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    {
        for key in ["SystemRoot", "WINDIR", "COMSPEC", "PATHEXT", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
    }
    for key in request
        .manifest
        .environment
        .required
        .iter()
        .chain(request.manifest.environment.optional.iter())
    {
        command.env_remove(key);
    }
    if request.allow_agent_custom_env {
        for key in &request.agent_profile.provider_config.custom_env_keys {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        for key in request
            .manifest
            .environment
            .required
            .iter()
            .chain(request.manifest.environment.optional.iter())
        {
            command.env_remove(key);
        }
    }
}

fn read_pipe<R>(mut reader: R) -> JoinHandle<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut output = Vec::new();
        let _ = reader.read_to_end(&mut output).await;
        output
    })
}

async fn await_pipe(handle: Option<JoinHandle<Vec<u8>>>) -> Vec<u8> {
    match handle {
        Some(handle) => handle.await.unwrap_or_default(),
        None => Vec::new(),
    }
}

fn output_events(stdout: Vec<u8>, stderr: Vec<u8>) -> Vec<RuntimeOutputEvent> {
    let mut events = Vec::new();
    if !stdout.is_empty() {
        events.push(RuntimeOutputEvent {
            kind: TaskEventType::ProcessStdout,
            payload: json!({"text": capped_text(&stdout), "stream": "stdout"}),
        });
    }
    if !stderr.is_empty() {
        events.push(RuntimeOutputEvent {
            kind: TaskEventType::ProcessStderr,
            payload: json!({"text": capped_text(&stderr), "stream": "stderr"}),
        });
    }
    events
}

fn capped_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_EVENT_BYTES)]).into_owned()
}

fn write_prompt_file(task_id: &str, prompt: &str) -> std::io::Result<PathBuf> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "opendaemon-{task_id}-prompt-{}",
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, prompt)?;
    Ok(path)
}

fn cleanup_temp_file(path: Option<&Path>) {
    if let Some(path) = path {
        let _ = std::fs::remove_file(path);
    }
}
