mod api;
mod cli;
mod registry;

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};

fn valid_manifest_json() -> Value {
    json!({
        "schema_version": "1",
        "id": "test-provider",
        "display_name": "Test Provider",
        "status": "community",
        "vendor": {
            "name": "Test Vendor",
            "homepage": "https://example.invalid",
            "support_url": null
        },
        "integration_type": "cli",
        "description": "Provider fixture for tests.",
        "install": {
            "macos": ["Install test-provider."],
            "linux": ["Install test-provider."],
            "windows": ["Install test-provider."]
        },
        "detect": {
            "commands": ["test-provider"],
            "version_args": ["--version"],
            "version_regex": null
        },
        "execution": {
            "command": "test-provider",
            "args": [],
            "input_mode": "stdin",
            "working_directory": "optional",
            "supports_streaming": false,
            "cancel_signal": "none"
        },
        "models": {
            "default": "test-model",
            "supported": ["test-model"]
        },
        "capabilities": {
            "filesystem_read": false,
            "filesystem_write": false,
            "shell": false,
            "git": false,
            "browser": false,
            "mcp": false,
            "remote_execution": false,
            "worktree": false,
            "direct_directory": false
        },
        "permissions": {
            "requires_directory_grant": false,
            "recommended_directory_lock": "none",
            "provider_permission_modes": ["default"],
            "supports_permission_events": false
        },
        "environment": {
            "required": [],
            "optional": []
        },
        "security": {
            "runs_locally": true,
            "sends_code_to_vendor": false,
            "data_policy_url": null,
            "review_level": "experimental"
        }
    })
}

fn temp_registry_with_provider(provider_dir: &str, manifest: Value) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new();
    let providers_dir = temp_dir.path().join("registry/providers");
    fs::create_dir_all(&providers_dir).unwrap();
    write_provider_fixture(&providers_dir, provider_dir, manifest);

    (temp_dir, providers_dir)
}

fn write_provider_fixture(providers_dir: &Path, provider_dir: &str, manifest: Value) {
    let dir = providers_dir.join(provider_dir);
    fs::create_dir_all(dir.join("examples")).unwrap();
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(dir.join("README.md"), "# Test Provider\n").unwrap();
    fs::write(
        dir.join("examples/basic.task.json"),
        r#"{"provider":"test-provider","prompt":"test"}"#,
    )
    .unwrap();
}

fn replace_manifest_field(
    providers_dir: &Path,
    provider_dir: &str,
    update: impl FnOnce(&mut Value),
) {
    let path = providers_dir.join(provider_dir).join("manifest.json");
    let mut manifest: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    update(&mut manifest);
    fs::write(path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
}

#[derive(Debug)]
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "opendaemon-registry-test-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&path).unwrap();

        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_suffix() -> u128 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    let counter = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    elapsed + u128::from(counter)
}
