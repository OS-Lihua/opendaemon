use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde_json::json;

use crate::{
    config::{RuntimeDetectionConfig, RuntimeEnvironment},
    registry::ProviderManifest,
    runtime::{
        detect::detect_provider,
        model::{RuntimeStatus, override_env_var_name, runtime_id},
        store::RuntimeStore,
    },
    tests::{TempDir, valid_manifest_json},
};

#[test]
fn runtime_identity_and_override_names_are_normalized() {
    assert_eq!(runtime_id("codex"), "rt_codex_local_cli");
    assert_eq!(
        runtime_id("generic-test-provider"),
        "rt_generic_test_provider_local_cli"
    );
    assert_eq!(
        runtime_id("Generic--Test.Provider"),
        "rt_generic_test_provider_local_cli"
    );
    assert_eq!(
        override_env_var_name("generic-test-provider"),
        "OPENDAEMON_PROVIDER_GENERIC_TEST_PROVIDER_PATH"
    );
}

#[tokio::test]
async fn runtime_store_returns_not_detected_until_updated_and_sorts_by_provider_id() {
    let store = RuntimeStore::default();
    let providers = [manifest_with_id("zeta"), manifest_with_id("alpha")];

    let initial = store.list_for_providers(&providers).await;
    let ids = initial
        .iter()
        .map(|runtime| runtime.provider_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["alpha", "zeta"]);
    assert!(initial.iter().all(|runtime| {
        runtime.status == RuntimeStatus::NotDetected && runtime.detected_at.is_none()
    }));

    store
        .save(crate::runtime::model::RuntimeView::available(
            "zeta",
            "/fake/zeta".into(),
            Some("1.0.0".to_owned()),
        ))
        .await;

    let updated = store.list_for_providers(&providers).await;
    let zeta = updated
        .iter()
        .find(|runtime| runtime.provider_id == "zeta")
        .unwrap();

    assert_eq!(zeta.status, RuntimeStatus::Available);
    assert_eq!(zeta.version.as_deref(), Some("1.0.0"));
    assert!(zeta.detected_at.is_some());
}

#[tokio::test]
async fn override_path_takes_precedence_over_path_search() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    let override_command =
        write_fake_command(temp_dir.path(), "override-provider", "echo override 9.9.9");
    let path_dir = temp_dir.path().join("path-bin");
    fs::create_dir_all(&path_dir).unwrap();
    write_fake_command(&path_dir, "test-provider", "echo path 1.0.0");

    let manifest = manifest_with_detect(
        "generic-test-provider",
        &["test-provider"],
        &["--version"],
        None,
    );
    let config = config_with_env([
        (
            override_env_var_name("generic-test-provider"),
            override_command.clone().into_os_string(),
        ),
        ("PATH".to_owned(), path_dir.into_os_string()),
    ]);

    let runtime = detect_provider(&manifest, &config).await;

    assert_eq!(runtime.status, RuntimeStatus::Available, "{runtime:#?}");
    assert_eq!(
        runtime.executable.as_deref(),
        Some(override_command.as_path())
    );
    assert_eq!(runtime.version.as_deref(), Some("override 9.9.9"));
}

#[tokio::test]
async fn invalid_override_reports_error_without_falling_back_to_path() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    let path_command = write_fake_command(temp_dir.path(), "test-provider", "echo path 1.0.0");
    let invalid_override = temp_dir.path().join("missing-provider");
    let manifest = manifest_with_detect(
        "generic-test-provider",
        &["test-provider"],
        &["--version"],
        None,
    );
    let config = config_with_env([
        (
            override_env_var_name("generic-test-provider"),
            invalid_override.into_os_string(),
        ),
        (
            "PATH".to_owned(),
            path_command.parent().unwrap().as_os_str().to_owned(),
        ),
    ]);

    let runtime = detect_provider(&manifest, &config).await;

    assert_eq!(runtime.status, RuntimeStatus::Error, "{runtime:#?}");
    assert_eq!(
        runtime.error.as_ref().unwrap().code,
        "override_not_executable"
    );
    assert!(runtime.executable.is_none());
    assert!(runtime.version.is_none());
}

#[tokio::test]
async fn path_commands_are_resolved_in_manifest_order() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    write_fake_command(temp_dir.path(), "first-provider", "echo first 1.0.0");
    write_fake_command(temp_dir.path(), "second-provider", "echo second 2.0.0");
    let manifest = manifest_with_detect(
        "test-provider",
        &["first-provider", "second-provider"],
        &["--version"],
        None,
    );

    let runtime = detect_provider(&manifest, &config_with_path(temp_dir.path())).await;

    assert_eq!(runtime.status, RuntimeStatus::Available, "{runtime:#?}");
    assert!(
        runtime
            .executable
            .as_ref()
            .unwrap()
            .ends_with(command_file_name("first-provider"))
    );
    assert_eq!(runtime.version.as_deref(), Some("first 1.0.0"));
}

#[tokio::test]
async fn missing_command_is_unavailable() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    let manifest =
        manifest_with_detect("test-provider", &["missing-provider"], &["--version"], None);

    let runtime = detect_provider(&manifest, &config_with_path(temp_dir.path())).await;

    assert_eq!(runtime.status, RuntimeStatus::Unavailable, "{runtime:#?}");
    assert_eq!(runtime.error.as_ref().unwrap().code, "command_not_found");
    assert!(runtime.executable.is_none());
}

#[tokio::test]
async fn version_probe_passes_args_and_parses_named_capture_from_stdout() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    let command = write_fake_command(
        temp_dir.path(),
        "test-provider",
        r#"if [ "$1" = "version" ] && [ "$2" = "--json" ]; then echo "tool v3.4.5"; else exit 9; fi"#,
    );
    let manifest = manifest_with_detect(
        "test-provider",
        &["test-provider"],
        &["version", "--json"],
        Some(r"v(?<version>\d+\.\d+\.\d+)"),
    );

    let runtime = detect_provider(&manifest, &config_with_path(temp_dir.path())).await;

    assert_eq!(runtime.status, RuntimeStatus::Available, "{runtime:#?}");
    assert_eq!(runtime.executable.as_deref(), Some(command.as_path()));
    assert_eq!(runtime.version.as_deref(), Some("3.4.5"));
}

#[tokio::test]
async fn version_probe_parses_first_capture_from_stderr() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    write_fake_command(
        temp_dir.path(),
        "test-provider",
        "echo 'provider 7.8.9' >&2",
    );
    let manifest = manifest_with_detect(
        "test-provider",
        &["test-provider"],
        &["--version"],
        Some(r"provider (\d+\.\d+\.\d+)"),
    );

    let runtime = detect_provider(&manifest, &config_with_path(temp_dir.path())).await;

    assert_eq!(runtime.status, RuntimeStatus::Available, "{runtime:#?}");
    assert_eq!(runtime.version.as_deref(), Some("7.8.9"));
}

#[tokio::test]
async fn version_probe_uses_first_non_empty_output_line_without_regex() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    write_fake_command(
        temp_dir.path(),
        "test-provider",
        "echo ''; echo '  provider 5.6.7  '",
    );
    let manifest = manifest_with_detect("test-provider", &["test-provider"], &["--version"], None);

    let runtime = detect_provider(&manifest, &config_with_path(temp_dir.path())).await;

    assert_eq!(runtime.status, RuntimeStatus::Available, "{runtime:#?}");
    assert_eq!(runtime.version.as_deref(), Some("provider 5.6.7"));
}

#[tokio::test]
async fn non_zero_version_probe_reports_provider_error() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    let command = write_fake_command(
        temp_dir.path(),
        "test-provider",
        "echo failure >&2; exit 42",
    );
    let manifest = manifest_with_detect("test-provider", &["test-provider"], &["--version"], None);

    let runtime = detect_provider(&manifest, &config_with_path(temp_dir.path())).await;

    assert_eq!(runtime.status, RuntimeStatus::Error, "{runtime:#?}");
    assert_eq!(runtime.executable.as_deref(), Some(command.as_path()));
    assert_eq!(
        runtime.error.as_ref().unwrap().code,
        "version_command_failed"
    );
}

#[tokio::test]
async fn timed_out_version_probe_reports_error_promptly() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    write_fake_command(temp_dir.path(), "test-provider", sleep_command_body(5));
    let manifest = manifest_with_detect("test-provider", &["test-provider"], &["--version"], None);
    let config = RuntimeDetectionConfig::default()
        .with_timeout(Duration::from_millis(100))
        .with_environment(RuntimeEnvironment::from_vars([(
            "PATH".to_owned(),
            temp_dir.path().as_os_str().to_owned(),
        )]));
    let started = Instant::now();

    let runtime = detect_provider(&manifest, &config).await;

    assert_eq!(runtime.status, RuntimeStatus::Error, "{runtime:#?}");
    assert_eq!(runtime.error.as_ref().unwrap().code, "version_timeout");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn empty_version_args_skip_probe_and_return_null_version() {
    let _guard = crate::tests::runtime_detection_test_guard().await;
    let temp_dir = TempDir::new();
    let command = write_fake_command(temp_dir.path(), "test-provider", "exit 99");
    let manifest = manifest_with_detect("test-provider", &["test-provider"], &[], None);

    let runtime = detect_provider(&manifest, &config_with_path(temp_dir.path())).await;

    assert_eq!(runtime.status, RuntimeStatus::Available, "{runtime:#?}");
    assert_eq!(runtime.executable.as_deref(), Some(command.as_path()));
    assert!(runtime.version.is_none());
}

fn manifest_with_id(provider_id: &str) -> ProviderManifest {
    manifest_with_detect(provider_id, &["test-provider"], &["--version"], None)
}

fn manifest_with_detect(
    provider_id: &str,
    commands: &[&str],
    version_args: &[&str],
    version_regex: Option<&str>,
) -> ProviderManifest {
    let mut manifest = valid_manifest_json();
    manifest["id"] = json!(provider_id);
    manifest["display_name"] = json!(provider_id);
    manifest["detect"]["commands"] = json!(commands);
    manifest["detect"]["version_args"] = json!(version_args);
    manifest["detect"]["version_regex"] =
        version_regex.map_or(serde_json::Value::Null, |regex| json!(regex));

    serde_json::from_value(manifest).unwrap()
}

fn config_with_path(path: &Path) -> RuntimeDetectionConfig {
    RuntimeDetectionConfig::default().with_environment(RuntimeEnvironment::from_vars([(
        "PATH".to_owned(),
        path.as_os_str().to_owned(),
    )]))
}

fn config_with_env<const N: usize>(vars: [(String, OsString); N]) -> RuntimeDetectionConfig {
    RuntimeDetectionConfig::default().with_environment(RuntimeEnvironment::from_vars(vars))
}

fn write_fake_command(dir: &Path, name: &str, body: impl AsRef<str>) -> PathBuf {
    let path = dir.join(command_file_name(name));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::write(&path, format!("#!/bin/sh\n{}\n", body.as_ref())).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
    }

    #[cfg(windows)]
    {
        fs::write(&path, format!("@echo off\r\n{}\r\n", body.as_ref())).unwrap();
    }

    path
}

fn command_file_name(name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{name}.cmd")
    }

    #[cfg(not(windows))]
    {
        name.to_owned()
    }
}

fn sleep_command_body(seconds: u64) -> String {
    #[cfg(windows)]
    {
        format!("ping -n {} 127.0.0.1 >NUL", seconds + 1)
    }

    #[cfg(not(windows))]
    {
        format!("sleep {seconds}")
    }
}
