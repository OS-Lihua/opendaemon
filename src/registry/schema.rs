use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};
use schemars::schema_for;
use serde_json::Value;

use super::manifest::ProviderManifest;

pub const PROVIDER_MANIFEST_SCHEMA_PATH: &str = "schemas/provider-manifest.schema.json";

pub fn default_schema_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(PROVIDER_MANIFEST_SCHEMA_PATH)
}

pub fn generated_schema_value() -> anyhow::Result<Value> {
    serde_json::to_value(schema_for!(ProviderManifest))
        .context("failed to serialize provider manifest schema")
}

pub fn generated_schema_json() -> anyhow::Result<String> {
    let schema = generated_schema_value()?;
    let mut json = serde_json::to_string_pretty(&schema)
        .context("failed to format provider manifest schema")?;
    json.push('\n');
    Ok(json)
}

pub fn validate_manifest_value(manifest_path: &Path, manifest: &Value) -> anyhow::Result<()> {
    let schema = generated_schema_value()?;
    let validator =
        jsonschema::validator_for(&schema).context("failed to compile provider manifest schema")?;
    let errors = validator
        .iter_errors(manifest)
        .map(|error| format!("{}: {error}", error.instance_path))
        .collect::<Vec<_>>();

    if errors.is_empty() {
        return Ok(());
    }

    bail!(
        "{} failed schema validation:\n{}",
        manifest_path.display(),
        errors.join("\n")
    );
}

pub fn check_schema_fresh(schema_path: &Path) -> anyhow::Result<()> {
    let expected = generated_schema_json()?;
    let actual = fs::read_to_string(schema_path)
        .with_context(|| format!("failed to read {}", schema_path.display()))?;

    if actual != expected {
        bail!(
            "{} is stale; regenerate it from ProviderManifest",
            schema_path.display()
        );
    }

    let schema_value: Value = serde_json::from_str(&actual)
        .with_context(|| format!("failed to parse {}", schema_path.display()))?;
    jsonschema::validator_for(&schema_value)
        .with_context(|| format!("failed to compile {}", schema_path.display()))?;

    Ok(())
}
