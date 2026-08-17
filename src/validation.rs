use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::model::{Manifest, Provenance};

pub const SUPPORTED_SCHEMA_VERSION: &str = "0.1.0";

const MANIFEST_SCHEMA: &str = include_str!("../schemas/manifest.schema.json");
const SESSION_SCHEMA: &str = include_str!("../schemas/session.schema.json");
const CONVERSATION_ITEM_SCHEMA: &str = include_str!("../schemas/conversation-item.schema.json");
const TASK_STATE_SCHEMA: &str = include_str!("../schemas/task-state.schema.json");
const WORKSPACE_STATE_SCHEMA: &str = include_str!("../schemas/workspace-state.schema.json");
const REPOSITORY_STATE_SCHEMA: &str = include_str!("../schemas/repository-state.schema.json");
const PROVENANCE_SCHEMA: &str = include_str!("../schemas/provenance.schema.json");

const REQUIRED_PACKAGE_FILES: [(&str, &str); 7] = [
    ("session.json", SESSION_SCHEMA),
    ("conversation.jsonl", CONVERSATION_ITEM_SCHEMA),
    ("task-state.json", TASK_STATE_SCHEMA),
    ("workspace-state.json", WORKSPACE_STATE_SCHEMA),
    ("repository-state.json", REPOSITORY_STATE_SCHEMA),
    ("handoff.md", ""),
    ("provenance.json", PROVENANCE_SCHEMA),
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

impl ValidationIssue {
    fn error(code: &str, path: impl Into<Option<String>>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Error,
            code: code.to_owned(),
            path: path.into(),
            message: message.into(),
        }
    }

    fn warning(code: &str, path: impl Into<Option<String>>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            code: code.to_owned(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == IssueSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == IssueSeverity::Warning)
            .count()
    }
}

pub fn validate_package(package_root: impl AsRef<Path>) -> ValidationReport {
    let root = package_root.as_ref();
    let mut issues = Vec::new();

    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            issues.push(ValidationIssue::error(
                "package.symlink",
                Some(root.display().to_string()),
                "package root must not be a symbolic link",
            ));
            return report(None, issues);
        }
        Ok(metadata) if !metadata.is_dir() => {
            issues.push(ValidationIssue::error(
                "package.not_directory",
                Some(root.display().to_string()),
                "package path is not a directory",
            ));
            return report(None, issues);
        }
        Ok(_) => {}
        Err(error) => {
            issues.push(ValidationIssue::error(
                "package.unreadable",
                Some(root.display().to_string()),
                format!("cannot read package directory: {error}"),
            ));
            return report(None, issues);
        }
    }

    let Some(manifest_value) = read_json(root, "manifest.json", &mut issues) else {
        return report(None, issues);
    };

    validate_value(
        MANIFEST_SCHEMA,
        &manifest_value,
        "manifest.json",
        &mut issues,
    );

    let manifest = match serde_json::from_value::<Manifest>(manifest_value) {
        Ok(manifest) => manifest,
        Err(error) => {
            issues.push(ValidationIssue::error(
                "manifest.decode",
                Some("manifest.json".to_owned()),
                format!("cannot decode manifest: {error}"),
            ));
            return report(None, issues);
        }
    };

    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        issues.push(ValidationIssue::error(
            "schema.unsupported",
            Some("manifest.json/schemaVersion".to_owned()),
            format!(
                "schema version {} is not supported; expected {SUPPORTED_SCHEMA_VERSION}",
                manifest.schema_version
            ),
        ));
    }

    validate_manifest_files(root, &manifest, &mut issues);
    validate_required_documents(root, &mut issues);
    validate_source_consistency(root, &manifest, &mut issues);
    warn_for_unlisted_files(root, &manifest, &mut issues);

    report(Some(manifest.schema_version), issues)
}

fn report(schema_version: Option<String>, issues: Vec<ValidationIssue>) -> ValidationReport {
    let valid = issues
        .iter()
        .all(|issue| issue.severity != IssueSeverity::Error);
    ValidationReport {
        valid,
        schema_version,
        issues,
    }
}

fn validate_manifest_files(root: &Path, manifest: &Manifest, issues: &mut Vec<ValidationIssue>) {
    let mut seen = HashSet::new();

    for entry in &manifest.files {
        if !seen.insert(entry.path.as_str()) {
            issues.push(ValidationIssue::error(
                "manifest.duplicate_path",
                Some(format!("manifest.json/files/{}", entry.path)),
                "manifest contains the same file path more than once",
            ));
            continue;
        }

        if !is_safe_relative_path(&entry.path) || entry.path == "manifest.json" {
            issues.push(ValidationIssue::error(
                "manifest.unsafe_path",
                Some(format!("manifest.json/files/{}", entry.path)),
                "manifest paths must be safe, relative package paths",
            ));
            continue;
        }

        if path_contains_symlink(root, &entry.path) {
            issues.push(ValidationIssue::error(
                "file.symlink",
                Some(entry.path.clone()),
                "manifest file path must not contain symbolic links",
            ));
            continue;
        }

        let full_path = root.join(&entry.path);
        let metadata = match fs::symlink_metadata(&full_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                issues.push(ValidationIssue::error(
                    "file.missing",
                    Some(entry.path.clone()),
                    format!("manifest file cannot be read: {error}"),
                ));
                continue;
            }
        };

        if metadata.file_type().is_symlink() {
            issues.push(ValidationIssue::error(
                "file.symlink",
                Some(entry.path.clone()),
                "manifest files must not be symbolic links",
            ));
            continue;
        }
        if !metadata.is_file() {
            issues.push(ValidationIssue::error(
                "file.not_regular",
                Some(entry.path.clone()),
                "manifest entry is not a regular file",
            ));
            continue;
        }

        match fs::read(&full_path) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if actual != entry.sha256 {
                    issues.push(ValidationIssue::error(
                        "file.checksum_mismatch",
                        Some(entry.path.clone()),
                        format!("SHA-256 mismatch: expected {}, got {actual}", entry.sha256),
                    ));
                }
            }
            Err(error) => issues.push(ValidationIssue::error(
                "file.unreadable",
                Some(entry.path.clone()),
                format!("cannot read manifest file: {error}"),
            )),
        }
    }

    for (required, _) in REQUIRED_PACKAGE_FILES {
        if !seen.contains(required) {
            issues.push(ValidationIssue::error(
                "manifest.required_file",
                Some("manifest.json/files".to_owned()),
                format!("required package file is not listed: {required}"),
            ));
        }
    }
}

fn validate_required_documents(root: &Path, issues: &mut Vec<ValidationIssue>) {
    for (path, schema) in REQUIRED_PACKAGE_FILES {
        if path == "conversation.jsonl" {
            validate_conversation(root, issues);
        } else if path == "handoff.md" {
            validate_handoff(root, issues);
        } else if let Some(value) = read_json(root, path, issues) {
            validate_value(schema, &value, path, issues);
        }
    }
}

fn validate_conversation(root: &Path, issues: &mut Vec<ValidationIssue>) {
    let path = root.join("conversation.jsonl");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            issues.push(ValidationIssue::error(
                "file.unreadable",
                Some("conversation.jsonl".to_owned()),
                format!("cannot read conversation: {error}"),
            ));
            return;
        }
    };

    let mut ids = HashSet::new();
    let mut parents = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let document = format!("conversation.jsonl:{line_number}");
        if line.trim().is_empty() {
            issues.push(ValidationIssue::error(
                "conversation.empty_line",
                Some(document),
                "JSON Lines documents must not contain blank lines",
            ));
            continue;
        }
        let value = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(error) => {
                issues.push(ValidationIssue::error(
                    "json.parse",
                    Some(document),
                    format!("invalid JSON: {error}"),
                ));
                continue;
            }
        };

        validate_value(CONVERSATION_ITEM_SCHEMA, &value, &document, issues);
        if let Some(id) = value.get("id").and_then(Value::as_str) {
            if !ids.insert(id.to_owned()) {
                issues.push(ValidationIssue::error(
                    "conversation.duplicate_id",
                    Some(document.clone()),
                    format!("duplicate conversation item id: {id}"),
                ));
            }
        }
        if let Some(parent_id) = value.get("parentId").and_then(Value::as_str) {
            parents.push((document, parent_id.to_owned()));
        }
    }

    for (document, parent_id) in parents {
        if !ids.contains(&parent_id) {
            issues.push(ValidationIssue::error(
                "conversation.parent_missing",
                Some(document),
                format!("parentId does not reference a conversation item: {parent_id}"),
            ));
        }
    }
}

fn validate_handoff(root: &Path, issues: &mut Vec<ValidationIssue>) {
    match fs::read_to_string(root.join("handoff.md")) {
        Ok(contents) if contents.trim().is_empty() => issues.push(ValidationIssue::error(
            "handoff.empty",
            Some("handoff.md".to_owned()),
            "handoff must contain continuation guidance",
        )),
        Ok(_) => {}
        Err(error) => issues.push(ValidationIssue::error(
            "file.unreadable",
            Some("handoff.md".to_owned()),
            format!("cannot read handoff: {error}"),
        )),
    }
}

fn validate_source_consistency(
    root: &Path,
    manifest: &Manifest,
    issues: &mut Vec<ValidationIssue>,
) {
    let Ok(contents) = fs::read_to_string(root.join("provenance.json")) else {
        return;
    };
    let Ok(provenance) = serde_json::from_str::<Provenance>(&contents) else {
        return;
    };
    if provenance.source != manifest.source {
        issues.push(ValidationIssue::error(
            "provenance.source_mismatch",
            Some("provenance.json/source".to_owned()),
            "provenance source must match the manifest source",
        ));
    }
}

fn validate_value(
    schema_source: &str,
    value: &Value,
    document: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let schema = match serde_json::from_str::<Value>(schema_source) {
        Ok(schema) => schema,
        Err(error) => {
            issues.push(ValidationIssue::error(
                "internal.schema_parse",
                None,
                format!("embedded schema cannot be parsed: {error}"),
            ));
            return;
        }
    };
    let validator = match jsonschema::validator_for(&schema) {
        Ok(validator) => validator,
        Err(error) => {
            issues.push(ValidationIssue::error(
                "internal.schema_compile",
                None,
                format!("embedded schema cannot be compiled: {error}"),
            ));
            return;
        }
    };

    for error in validator.iter_errors(value) {
        let instance_path = error.instance_path().to_string();
        let path = if instance_path.is_empty() {
            document.to_owned()
        } else {
            format!("{document}{instance_path}")
        };
        issues.push(ValidationIssue::error(
            "schema.validation",
            Some(path),
            error.to_string(),
        ));
    }
}

fn read_json(root: &Path, relative: &str, issues: &mut Vec<ValidationIssue>) -> Option<Value> {
    let path = root.join(relative);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            issues.push(ValidationIssue::error(
                "file.unreadable",
                Some(relative.to_owned()),
                format!("cannot read JSON document: {error}"),
            ));
            return None;
        }
    };

    match serde_json::from_str(&contents) {
        Ok(value) => Some(value),
        Err(error) => {
            issues.push(ValidationIssue::error(
                "json.parse",
                Some(relative.to_owned()),
                format!("invalid JSON: {error}"),
            ));
            None
        }
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn path_contains_symlink(root: &Path, relative: &str) -> bool {
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return true,
            Ok(_) => {}
            // Missing paths are reported by the regular file check.
            Err(_) => return false,
        }
    }
    false
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        // Writing to a String is infallible.
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn warn_for_unlisted_files(root: &Path, manifest: &Manifest, issues: &mut Vec<ValidationIssue>) {
    let listed: HashSet<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let mut discovered = HashMap::new();
    collect_files(root, root, &mut discovered);

    for (relative, path) in discovered {
        if relative != "manifest.json" && !listed.contains(relative.as_str()) {
            issues.push(ValidationIssue::warning(
                "manifest.unlisted_file",
                Some(relative),
                format!("package contains an unlisted file: {}", path.display()),
            ));
        }
    }
}

fn collect_files(root: &Path, directory: &Path, files: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(root, &path, files);
        } else if file_type.is_file()
            && let Ok(relative) = path.strip_prefix(root)
        {
            files.insert(relative.to_string_lossy().replace('\\', "/"), path);
        }
    }
}
