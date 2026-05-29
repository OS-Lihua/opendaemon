use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::bail;

use super::{ProviderEntry, manifest::ProviderManifest};

pub fn validate_manifest_entry(
    provider_dir_name: &str,
    provider_dir: &Path,
    manifest: &ProviderManifest,
) -> Vec<String> {
    let mut errors = Vec::new();
    let provider = manifest.id.as_str();

    if manifest.id != provider_dir_name {
        errors.push(format!(
            "provider {provider}: manifest id does not match directory name {provider_dir_name}"
        ));
    }

    require_file(
        provider,
        provider_dir.join("README.md").as_path(),
        &mut errors,
    );
    require_file(
        provider,
        provider_dir.join("examples/basic.task.json").as_path(),
        &mut errors,
    );

    validate_manifest_fields(manifest, &mut errors);

    errors
}

pub fn validate_registry_entries(entries: &[ProviderEntry]) -> anyhow::Result<()> {
    let mut errors = Vec::new();
    let mut seen = HashMap::<&str, &Path>::new();

    for entry in entries {
        if let Some(first_path) = seen.insert(&entry.manifest.id, &entry.manifest_path) {
            errors.push(format!(
                "provider {}: duplicate provider id in {} and {}",
                entry.manifest.id,
                first_path.display(),
                entry.manifest_path.display()
            ));
        }
    }

    if errors.is_empty() {
        return Ok(());
    }

    bail!("registry validation failed:\n{}", errors.join("\n"))
}

fn validate_manifest_fields(manifest: &ProviderManifest, errors: &mut Vec<String>) {
    let provider = manifest.id.as_str();

    require_non_empty(provider, "schema_version", &manifest.schema_version, errors);
    require_non_empty(provider, "id", &manifest.id, errors);
    require_non_empty(provider, "display_name", &manifest.display_name, errors);
    require_non_empty(provider, "vendor.name", &manifest.vendor.name, errors);
    require_non_empty(
        provider,
        "vendor.homepage",
        &manifest.vendor.homepage,
        errors,
    );
    require_non_empty(provider, "description", &manifest.description, errors);
    require_non_empty(
        provider,
        "execution.command",
        &manifest.execution.command,
        errors,
    );
    require_non_empty(provider, "models.default", &manifest.models.default, errors);

    if manifest.detect.commands.is_empty() {
        errors.push(format!(
            "provider {provider}: detect.commands must not be empty"
        ));
    }

    if is_absolute_command(&manifest.execution.command) {
        errors.push(format!(
            "provider {provider}: execution.command must not be an absolute path"
        ));
    }

    for (index, command) in manifest.detect.commands.iter().enumerate() {
        if command.trim().is_empty() {
            errors.push(format!(
                "provider {provider}: detect.commands[{index}] must not be empty"
            ));
        }

        if is_absolute_command(command) {
            errors.push(format!(
                "provider {provider}: detect.commands[{index}] must not be an absolute path"
            ));
        }
    }

    if !manifest
        .models
        .supported
        .iter()
        .any(|model| model == &manifest.models.default)
    {
        errors.push(format!(
            "provider {provider}: models.default must be present in models.supported"
        ));
    }

    push_duplicate_errors(
        provider,
        "models.supported",
        &manifest.models.supported,
        errors,
    );
    push_duplicate_errors(
        provider,
        "environment.required",
        &manifest.environment.required,
        errors,
    );
    push_duplicate_errors(
        provider,
        "environment.optional",
        &manifest.environment.optional,
        errors,
    );
    push_duplicate_errors(
        provider,
        "permissions.provider_permission_modes",
        &manifest.permissions.provider_permission_modes,
        errors,
    );

    let required_env = manifest.environment.required.iter().collect::<HashSet<_>>();
    for name in &manifest.environment.optional {
        if required_env.contains(name) {
            errors.push(format!(
                "provider {provider}: environment variable {name} appears in both required and optional"
            ));
        }
    }

    if manifest.capabilities.remote_execution && !manifest.security.sends_code_to_vendor {
        errors.push(format!(
            "provider {provider}: remote_execution requires security.sends_code_to_vendor"
        ));
    }
}

fn require_file(provider: &str, path: &Path, errors: &mut Vec<String>) {
    if !path.is_file() {
        errors.push(format!(
            "provider {provider}: required file missing at {}",
            path.display()
        ));
    }
}

fn require_non_empty(provider: &str, field: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("provider {provider}: {field} must not be empty"));
    }
}

fn push_duplicate_errors(provider: &str, field: &str, values: &[String], errors: &mut Vec<String>) {
    let mut seen = HashSet::new();
    for value in values {
        if value.trim().is_empty() {
            errors.push(format!(
                "provider {provider}: {field} must not contain empty values"
            ));
        }

        if !seen.insert(value) {
            errors.push(format!(
                "provider {provider}: {field} contains duplicate value {value}"
            ));
        }
    }
}

fn is_absolute_command(command: &str) -> bool {
    let command = command.trim();
    Path::new(command).is_absolute()
        || command.starts_with('\\')
        || command
            .as_bytes()
            .get(1)
            .is_some_and(|separator| *separator == b':')
}
