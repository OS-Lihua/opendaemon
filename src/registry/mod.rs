pub mod manifest;
pub mod schema;
mod validate;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;

pub use manifest::{
    AcpConfig, AcpTransport, AcpWorkingDirectoryMode, CancelSignal, DetectConfig,
    DirectoryLockMode, EnvironmentConfig, ExecutionConfig, ExecutionInputMode, InstallInstructions,
    IntegrationType, ModelConfig, ProviderCapabilities, ProviderManifest, ProviderPermissions,
    ProviderStatus, SecurityConfig, SecurityReviewLevel, VendorInfo, WorkingDirectoryMode,
};

pub const PROVIDERS_REGISTRY_PATH: &str = "registry/providers";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEntry {
    pub directory_name: String,
    pub manifest_path: PathBuf,
    pub manifest: ProviderManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistry {
    providers: Vec<ProviderEntry>,
}

impl ProviderRegistry {
    #[must_use]
    pub fn providers(&self) -> &[ProviderEntry] {
        &self.providers
    }

    #[must_use]
    pub fn get(&self, provider_id: &str) -> Option<&ProviderEntry> {
        self.providers
            .iter()
            .find(|provider| provider.manifest.id == provider_id)
    }
}

pub fn default_providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(PROVIDERS_REGISTRY_PATH)
}

pub fn default_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn load_default_registry() -> anyhow::Result<ProviderRegistry> {
    load_registry_from_dir(&default_providers_dir())
}

pub fn load_registry_from_dir(providers_dir: &Path) -> anyhow::Result<ProviderRegistry> {
    let mut errors = Vec::new();
    let mut providers = Vec::new();

    let entries = fs::read_dir(providers_dir)
        .with_context(|| format!("failed to read {}", providers_dir.display()))?;

    for entry in entries {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", providers_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;

        if !file_type.is_dir() {
            continue;
        }

        let provider_dir = entry.path();
        let directory_name = entry.file_name().to_string_lossy().into_owned();
        let manifest_path = provider_dir.join("manifest.json");

        match load_manifest_file(&manifest_path) {
            Ok(manifest) => {
                errors.extend(validate::validate_manifest_entry(
                    &directory_name,
                    &provider_dir,
                    &manifest,
                ));
                providers.push(ProviderEntry {
                    directory_name,
                    manifest_path,
                    manifest,
                });
            }
            Err(error) => errors.push(error.to_string()),
        }
    }

    if let Err(error) = validate::validate_registry_entries(&providers) {
        errors.push(error.to_string());
    }

    if !errors.is_empty() {
        anyhow::bail!("registry validation failed:\n{}", errors.join("\n"));
    }

    providers.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));

    Ok(ProviderRegistry { providers })
}

pub fn check_default_registry() -> anyhow::Result<()> {
    check_registry(&default_repo_root())
}

pub fn check_registry(repo_root: &Path) -> anyhow::Result<()> {
    load_registry_from_dir(&repo_root.join(PROVIDERS_REGISTRY_PATH))?;
    schema::check_schema_fresh(&repo_root.join(schema::PROVIDER_MANIFEST_SCHEMA_PATH))
}

fn load_manifest_file(manifest_path: &Path) -> anyhow::Result<ProviderManifest> {
    let contents = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

    schema::validate_manifest_value(manifest_path, &value)?;

    serde_json::from_value(value)
        .with_context(|| format!("failed to deserialize {}", manifest_path.display()))
}
