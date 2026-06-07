use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use regex::Regex;
use tokio::{io::AsyncReadExt, process::Command, task::JoinHandle, time};

use crate::{
    config::{RuntimeDetectionConfig, RuntimeEnvironment},
    registry::{AcpTransport, IntegrationType, ProviderManifest},
};

use super::model::{RuntimeError, RuntimeKind, RuntimeView, override_env_var_name};

pub async fn detect_provider(
    manifest: &ProviderManifest,
    config: &RuntimeDetectionConfig,
) -> RuntimeView {
    match manifest.integration_type {
        IntegrationType::Cli => {}
        IntegrationType::Acp => return detect_acp_provider(manifest, config).await,
        IntegrationType::Http => {
            return detect_http_provider(manifest);
        }
        IntegrationType::Native => {
            return RuntimeView::error(
                manifest.id.clone(),
                None,
                RuntimeError::new(
                    "unsupported_provider_integration",
                    "provider integration is not supported by local runtime detection",
                ),
            );
        }
    }

    let executable = match resolve_executable(manifest, &config.environment) {
        Ok(executable) => executable,
        Err(error) => return error.into_runtime_view(&manifest.id),
    };

    if manifest.detect.version_args.is_empty() {
        return RuntimeView::available(manifest.id.clone(), executable, None);
    }

    match run_version_probe(manifest, &executable, config).await {
        Ok(version) => RuntimeView::available(manifest.id.clone(), executable, Some(version)),
        Err(error) => RuntimeView::error(manifest.id.clone(), Some(executable), error),
    }
}

pub async fn detect_providers(
    providers: &[ProviderManifest],
    config: &RuntimeDetectionConfig,
) -> Vec<RuntimeView> {
    let mut runtimes = Vec::new();

    for provider in providers.iter().filter(|provider| {
        matches!(
            provider.integration_type,
            IntegrationType::Cli | IntegrationType::Acp | IntegrationType::Http
        )
    }) {
        runtimes.push(detect_provider(provider, config).await);
    }

    runtimes.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
    runtimes
}

fn detect_http_provider(manifest: &ProviderManifest) -> RuntimeView {
    let Some(http) = &manifest.http else {
        return RuntimeView::error_with_kind(
            manifest.id.clone(),
            RuntimeKind::RemoteHttp,
            None,
            RuntimeError::new(
                "http_invalid_configuration",
                "provider http configuration is missing",
            ),
        );
    };

    if http.endpoint.trim().is_empty() {
        return RuntimeView::error_with_kind(
            manifest.id.clone(),
            RuntimeKind::RemoteHttp,
            None,
            RuntimeError::new(
                "http_invalid_configuration",
                "provider http endpoint is missing",
            ),
        );
    }

    RuntimeView::available_with_kind(
        manifest.id.clone(),
        RuntimeKind::RemoteHttp,
        PathBuf::from(http.endpoint.clone()),
        None,
    )
}

async fn detect_acp_provider(
    manifest: &ProviderManifest,
    config: &RuntimeDetectionConfig,
) -> RuntimeView {
    let Some(acp) = &manifest.acp else {
        return RuntimeView::error_with_kind(
            manifest.id.clone(),
            RuntimeKind::LocalAcp,
            None,
            RuntimeError::new(
                "acp_invalid_configuration",
                "provider acp configuration is missing",
            ),
        );
    };

    match acp.transport {
        AcpTransport::Stdio => {
            let Some(command) = acp.command.as_ref().and_then(|segments| segments.first()) else {
                return RuntimeView::error_with_kind(
                    manifest.id.clone(),
                    RuntimeKind::LocalAcp,
                    None,
                    RuntimeError::new(
                        "acp_invalid_configuration",
                        "acp stdio transport requires a command",
                    ),
                );
            };

            match resolve_path_command(command, &config.environment) {
                Some(executable) => RuntimeView::available_with_kind(
                    manifest.id.clone(),
                    RuntimeKind::LocalAcp,
                    executable,
                    None,
                ),
                None => RuntimeView::unavailable_with_kind(
                    manifest.id.clone(),
                    RuntimeKind::LocalAcp,
                    RuntimeError::new(
                        "acp_runtime_unavailable",
                        "no configured acp command was found",
                    ),
                ),
            }
        }
        AcpTransport::LocalSocket => {
            if acp.endpoint.as_deref().is_some() {
                RuntimeView::available_with_kind(
                    manifest.id.clone(),
                    RuntimeKind::LocalAcp,
                    PathBuf::from("acp://local-socket"),
                    None,
                )
            } else {
                RuntimeView::error_with_kind(
                    manifest.id.clone(),
                    RuntimeKind::LocalAcp,
                    None,
                    RuntimeError::new(
                        "acp_invalid_configuration",
                        "acp local_socket transport requires an endpoint",
                    ),
                )
            }
        }
    }
}

fn resolve_executable(
    manifest: &ProviderManifest,
    environment: &RuntimeEnvironment,
) -> Result<PathBuf, ResolveExecutableError> {
    let override_name = override_env_var_name(&manifest.id);

    if let Some(override_path) = environment.var_os(&override_name) {
        let path = PathBuf::from(override_path);

        if is_executable(&path) {
            return Ok(path);
        }

        return Err(ResolveExecutableError::OverrideNotExecutable);
    }

    for command in &manifest.detect.commands {
        if let Some(executable) = resolve_path_command(command, environment) {
            return Ok(executable);
        }
    }

    Err(ResolveExecutableError::CommandNotFound)
}

enum ResolveExecutableError {
    OverrideNotExecutable,
    CommandNotFound,
}

impl ResolveExecutableError {
    fn into_runtime_view(self, provider_id: &str) -> RuntimeView {
        match self {
            Self::OverrideNotExecutable => RuntimeView::error(
                provider_id.to_owned(),
                None,
                RuntimeError::new(
                    "override_not_executable",
                    "provider executable override is not an executable file",
                ),
            ),
            Self::CommandNotFound => RuntimeView::unavailable(
                provider_id.to_owned(),
                RuntimeError::new(
                    "command_not_found",
                    "no configured detect command was found",
                ),
            ),
        }
    }
}

fn resolve_path_command(command: &str, environment: &RuntimeEnvironment) -> Option<PathBuf> {
    match environment {
        RuntimeEnvironment::System => which::which(command).ok(),
        RuntimeEnvironment::Map(_) => {
            let path = environment.path()?;
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            which::which_in(command, Some(path), cwd).ok()
        }
    }
}

async fn run_version_probe(
    manifest: &ProviderManifest,
    executable: &Path,
    config: &RuntimeDetectionConfig,
) -> Result<String, RuntimeError> {
    let mut command = Command::new(executable);
    command
        .args(&manifest.detect.version_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    match &config.environment {
        RuntimeEnvironment::System => {}
        RuntimeEnvironment::Map(vars) => {
            command.env_clear();
            command.envs(vars);
        }
    }

    for env_key in manifest
        .environment
        .required
        .iter()
        .chain(manifest.environment.optional.iter())
    {
        command.env_remove(env_key);
    }

    let mut child = command.spawn().map_err(|error| {
        RuntimeError::new(
            "version_command_failed",
            format!("failed to start version command: {error}"),
        )
    })?;

    let stdout = child.stdout.take().map(read_pipe);
    let stderr = child.stderr.take().map(read_pipe);

    let status = match wait_for_child(&mut child, config.timeout).await {
        Ok(status) => status,
        Err(WaitError::Wait(error)) => {
            abort_pipe(stdout);
            abort_pipe(stderr);
            return Err(RuntimeError::new(
                "version_command_failed",
                format!("failed to wait for version command: {error}"),
            ));
        }
        Err(WaitError::Timeout) => {
            let _ = child.kill().await;
            abort_pipe(stdout);
            abort_pipe(stderr);
            return Err(RuntimeError::new(
                "version_timeout",
                "version command timed out",
            ));
        }
    };

    let stdout = await_pipe(stdout).await;
    let stderr = await_pipe(stderr).await;

    if !status.success() {
        return Err(RuntimeError::new(
            "version_command_failed",
            format!("version command exited with status {status}"),
        ));
    }

    parse_version(&manifest.detect.version_regex, &stdout, &stderr).ok_or_else(|| {
        RuntimeError::new(
            "version_parse_failed",
            "version output did not match the configured parser",
        )
    })
}

async fn wait_for_child(
    child: &mut tokio::process::Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, WaitError> {
    let sleep = time::sleep(timeout);
    tokio::pin!(sleep);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => return Err(WaitError::Wait(error)),
        }

        tokio::select! {
            () = &mut sleep => return Err(WaitError::Timeout),
            () = time::sleep(Duration::from_millis(10)) => {}
        }
    }
}

enum WaitError {
    Wait(std::io::Error),
    Timeout,
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

fn abort_pipe(handle: Option<JoinHandle<Vec<u8>>>) {
    if let Some(handle) = handle {
        handle.abort();
    }
}

fn parse_version(version_regex: &Option<String>, stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let output = combined_output(stdout, stderr);

    if let Some(pattern) = version_regex {
        let regex = Regex::new(pattern).ok()?;
        let captures = regex.captures(&output)?;
        let matched = captures
            .name("version")
            .or_else(|| captures.get(1))
            .or_else(|| captures.get(0))?;
        let version = matched.as_str().trim();

        return (!version.is_empty()).then(|| version.to_owned());
    }

    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut output = String::new();
    output.push_str(&String::from_utf8_lossy(stdout));

    if !stdout.is_empty() && !stderr.is_empty() {
        output.push('\n');
    }

    output.push_str(&String::from_utf8_lossy(stderr));
    output
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        path.metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        true
    }
}
