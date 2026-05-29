use std::fs;

use serde_json::json;

use crate::{
    registry::{self, ProviderManifest},
    tests::{
        replace_manifest_field, temp_registry_with_provider, valid_manifest_json,
        write_provider_fixture,
    },
};

#[test]
fn valid_manifest_deserializes() {
    let manifest: ProviderManifest = serde_json::from_value(valid_manifest_json()).unwrap();

    assert_eq!(manifest.id, "test-provider");
    assert_eq!(manifest.models.default, "test-model");
}

#[test]
fn manifest_missing_required_field_fails() {
    let mut manifest = valid_manifest_json();
    manifest.as_object_mut().unwrap().remove("display_name");

    let error = serde_json::from_value::<ProviderManifest>(manifest).unwrap_err();

    assert!(error.to_string().contains("display_name"));
}

#[test]
fn manifest_unknown_enum_value_fails() {
    let mut manifest = valid_manifest_json();
    manifest["status"] = json!("unknown");

    let error = serde_json::from_value::<ProviderManifest>(manifest).unwrap_err();

    assert!(error.to_string().contains("unknown variant"));
}

#[test]
fn manifest_unknown_field_fails() {
    let mut manifest = valid_manifest_json();
    manifest["unexpected"] = json!(true);

    let error = serde_json::from_value::<ProviderManifest>(manifest).unwrap_err();

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn committed_provider_fixtures_load_sorted() {
    let registry = registry::load_default_registry().unwrap();
    let ids = registry
        .providers()
        .iter()
        .map(|entry| entry.manifest.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["claude", "codex", "generic-test-provider"]);
}

#[test]
fn schema_matches_committed_schema() {
    registry::schema::check_schema_fresh(&registry::schema::default_schema_path()).unwrap();
}

#[test]
fn registry_check_succeeds_for_committed_fixtures() {
    registry::check_default_registry().unwrap();
}

#[test]
fn validation_rejects_default_model_not_supported() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["models"]["default"] = json!("missing-model")
    });

    assert_registry_error_contains(&providers_dir, "models.default");
}

#[test]
fn validation_rejects_duplicate_models() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["models"]["supported"] = json!(["test-model", "test-model"]);
    });

    assert_registry_error_contains(&providers_dir, "duplicate value test-model");
}

#[test]
fn validation_rejects_duplicate_environment_values() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["environment"]["required"] = json!(["A", "A"]);
    });

    assert_registry_error_contains(&providers_dir, "environment.required");
}

#[test]
fn validation_rejects_environment_required_optional_overlap() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["environment"]["required"] = json!(["SHARED"]);
        manifest["environment"]["optional"] = json!(["SHARED"]);
    });

    assert_registry_error_contains(&providers_dir, "appears in both required and optional");
}

#[test]
fn validation_rejects_duplicate_permission_modes() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["permissions"]["provider_permission_modes"] = json!(["default", "default"]);
    });

    assert_registry_error_contains(&providers_dir, "provider_permission_modes");
}

#[test]
fn validation_rejects_empty_required_fields() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["display_name"] = json!("");
    });

    assert_registry_error_contains(&providers_dir, "display_name must not be empty");
}

#[test]
fn validation_rejects_absolute_execution_command() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["execution"]["command"] = json!("/usr/bin/test-provider");
    });

    assert_registry_error_contains(&providers_dir, "execution.command");
}

#[test]
fn validation_rejects_absolute_detect_command() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["detect"]["commands"] = json!(["/usr/bin/test-provider"]);
    });

    assert_registry_error_contains(&providers_dir, "detect.commands[0]");
}

#[test]
fn validation_rejects_manifest_id_directory_mismatch() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("directory-provider", valid_manifest_json());

    assert_registry_error_contains(&providers_dir, "does not match directory name");
}

#[test]
fn validation_rejects_duplicate_provider_ids() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    write_provider_fixture(&providers_dir, "duplicate-provider", valid_manifest_json());

    assert_registry_error_contains(&providers_dir, "duplicate provider id");
}

#[test]
fn validation_rejects_missing_readme() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    fs::remove_file(providers_dir.join("test-provider/README.md")).unwrap();

    assert_registry_error_contains(&providers_dir, "README.md");
}

#[test]
fn validation_rejects_missing_basic_task_example() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    fs::remove_file(
        providers_dir
            .join("test-provider")
            .join("examples/basic.task.json"),
    )
    .unwrap();

    assert_registry_error_contains(&providers_dir, "basic.task.json");
}

#[test]
fn validation_rejects_remote_execution_without_vendor_upload_disclosure() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    replace_manifest_field(&providers_dir, "test-provider", |manifest| {
        manifest["capabilities"]["remote_execution"] = json!(true);
        manifest["security"]["sends_code_to_vendor"] = json!(false);
    });

    assert_registry_error_contains(&providers_dir, "remote_execution");
}

#[test]
fn validation_rejects_invalid_json() {
    let (_temp_dir, providers_dir) =
        temp_registry_with_provider("test-provider", valid_manifest_json());
    fs::write(
        providers_dir.join("test-provider/manifest.json"),
        "{ invalid json",
    )
    .unwrap();

    assert_registry_error_contains(&providers_dir, "failed to parse");
}

fn assert_registry_error_contains(providers_dir: &std::path::Path, expected: &str) {
    let error = registry::load_registry_from_dir(providers_dir).unwrap_err();
    let message = error.to_string();

    assert!(
        message.contains(expected),
        "expected {message:?} to contain {expected:?}"
    );
}
