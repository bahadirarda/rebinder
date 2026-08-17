use std::{
    fs,
    path::{Path, PathBuf},
};

use rebinder::{inspect_package, validate_package};
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn validates_the_example_package() {
    let report = validate_package(example_package());

    assert!(report.valid, "validation issues: {:?}", report.issues);
    assert_eq!(report.schema_version.as_deref(), Some("0.1.0"));
    assert_eq!(report.error_count(), 0);
}

#[test]
fn detects_file_tampering() {
    let package = copied_example();
    let handoff = package.path().join("handoff.md");
    fs::write(handoff, "tampered\n").expect("write tampered handoff");

    let report = validate_package(package.path());

    assert!(!report.valid);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "file.checksum_mismatch")
    );
}

#[test]
fn rejects_unsafe_manifest_paths() {
    let package = copied_example();
    let manifest_path = package.path().join("manifest.json");
    let mut manifest: Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("decode manifest");
    manifest["files"][0]["path"] = Value::String("../session.json".to_owned());
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");

    let report = validate_package(package.path());

    assert!(!report.valid);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "manifest.unsafe_path")
    );
}

#[test]
fn rejects_missing_conversation_parents() {
    let package = copied_example();
    let conversation_path = package.path().join("conversation.jsonl");
    let conversation = fs::read_to_string(&conversation_path).expect("read conversation");
    fs::write(
        conversation_path,
        conversation.replace("\"parentId\":\"message-1\"", "\"parentId\":\"missing\""),
    )
    .expect("write conversation");

    let report = validate_package(package.path());

    assert!(!report.valid);
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "conversation.parent_missing")
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_in_manifest_path_components() {
    use std::os::unix::fs::symlink;

    let package = copied_example();
    let outside = tempfile::tempdir().expect("create outside directory");
    fs::write(outside.path().join("leak.patch"), "secret\n").expect("write outside file");
    symlink(outside.path(), package.path().join("patches")).expect("create directory symlink");

    let manifest_path = package.path().join("manifest.json");
    let mut manifest: Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("decode manifest");
    manifest["files"]
        .as_array_mut()
        .expect("manifest files")
        .push(serde_json::json!({
            "path": "patches/leak.patch",
            "mediaType": "text/x-diff",
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        }));
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("encode manifest"),
    )
    .expect("write manifest");

    let report = validate_package(package.path());

    assert!(!report.valid);
    assert!(report.issues.iter().any(|issue| {
        issue.code == "file.symlink" && issue.path.as_deref() == Some("patches/leak.patch")
    }));
}

#[test]
fn inspects_portable_state() {
    let inspection = inspect_package(example_package()).expect("inspect example package");
    let summary = inspection.summary.expect("valid package summary");

    assert_eq!(summary.source.provider, "codex");
    assert_eq!(summary.conversation.item_count, 4);
    assert_eq!(summary.task.completed_steps, 1);
    assert_eq!(summary.repository.changes, 1);
    assert_eq!(summary.provenance.redacted_values, 2);
}

fn example_package() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal-session")
}

fn copied_example() -> TempDir {
    let temporary = tempfile::tempdir().expect("create temporary package");
    copy_directory(&example_package(), temporary.path());
    temporary
}

fn copy_directory(source: &Path, target: &Path) {
    for entry in fs::read_dir(source).expect("read example package") {
        let entry = entry.expect("read directory entry");
        let destination = target.join(entry.file_name());
        if entry.file_type().expect("read file type").is_dir() {
            fs::create_dir(&destination).expect("create copied directory");
            copy_directory(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy package file");
        }
    }
}
