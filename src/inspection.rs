use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::{
    model::{
        ConversationItem, ConversationSummary, Manifest, PlanStepStatus, Provenance,
        RepositoryState, Session, SourceDescriptor, TaskState, WorkspaceState,
    },
    validation::{ValidationReport, validate_package},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub validation: ValidationReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<PackageSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub package: String,
    pub schema_version: String,
    pub source: SourceDescriptor,
    pub session: SessionSummary,
    pub conversation: ConversationSummary,
    pub task: TaskSummary,
    pub workspace: WorkspaceSummary,
    pub repository: RepositorySummary,
    pub provenance: ProvenanceSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub intent: String,
    pub status: String,
    pub plan_steps: usize,
    pub completed_steps: usize,
    pub open_questions: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub cwd: String,
    pub roots: usize,
    pub files: usize,
    pub environment_entries: usize,
    pub redacted_environment_entries: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummary {
    pub repositories: usize,
    pub changes: usize,
    pub branches: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceSummary {
    pub exported_at: String,
    pub transformations: usize,
    pub redaction_events: usize,
    pub redacted_values: u64,
}

#[derive(Debug, Error)]
pub enum InspectionError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot decode {path}: {source}")]
    Decode {
        path: PathBuf,
        source: serde_json::Error,
    },
}

pub fn inspect_package(package_root: impl AsRef<Path>) -> Result<Inspection, InspectionError> {
    let root = package_root.as_ref();
    let validation = validate_package(root);
    if !validation.valid {
        return Ok(Inspection {
            validation,
            summary: None,
        });
    }

    let manifest: Manifest = read_json(root.join("manifest.json"))?;
    let session: Session = read_json(root.join("session.json"))?;
    let task: TaskState = read_json(root.join("task-state.json"))?;
    let workspace: WorkspaceState = read_json(root.join("workspace-state.json"))?;
    let repository: RepositoryState = read_json(root.join("repository-state.json"))?;
    let provenance: Provenance = read_json(root.join("provenance.json"))?;
    let conversation = read_conversation(root.join("conversation.jsonl"))?;

    let mut roles = BTreeMap::new();
    for item in &conversation {
        *roles.entry(item.role.as_str().to_owned()).or_insert(0) += 1;
    }

    let summary = PackageSummary {
        package: root.display().to_string(),
        schema_version: manifest.schema_version,
        source: manifest.source,
        session: SessionSummary {
            id: session.id,
            title: session.title,
            updated_at: session.updated_at,
        },
        conversation: ConversationSummary {
            item_count: conversation.len(),
            roles,
        },
        task: TaskSummary {
            intent: task.intent,
            status: task.status.as_str().to_owned(),
            plan_steps: task.plan.len(),
            completed_steps: task
                .plan
                .iter()
                .filter(|step| step.status == PlanStepStatus::Completed)
                .count(),
            open_questions: task.open_questions.len(),
        },
        workspace: WorkspaceSummary {
            cwd: workspace.cwd,
            roots: workspace.roots.len(),
            files: workspace.files.len(),
            environment_entries: workspace.environment.len(),
            redacted_environment_entries: workspace
                .environment
                .iter()
                .filter(|entry| entry.redacted)
                .count(),
        },
        repository: RepositorySummary {
            repositories: repository.repositories.len(),
            changes: repository
                .repositories
                .iter()
                .map(|repository| repository.changes.len())
                .sum(),
            branches: repository
                .repositories
                .iter()
                .filter_map(|repository| repository.head.branch.clone())
                .collect(),
        },
        provenance: ProvenanceSummary {
            exported_at: provenance.exported_at,
            transformations: provenance.transformations.len(),
            redaction_events: provenance.redactions.len(),
            redacted_values: provenance
                .redactions
                .iter()
                .map(|redaction| redaction.count)
                .sum(),
        },
    };

    Ok(Inspection {
        validation,
        summary: Some(summary),
    })
}

fn read_json<T: DeserializeOwned>(path: PathBuf) -> Result<T, InspectionError> {
    let contents = fs::read_to_string(&path).map_err(|source| InspectionError::Read {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| InspectionError::Decode { path, source })
}

fn read_conversation(path: PathBuf) -> Result<Vec<ConversationItem>, InspectionError> {
    let contents = fs::read_to_string(&path).map_err(|source| InspectionError::Read {
        path: path.clone(),
        source,
    })?;
    contents
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| InspectionError::Decode { path, source })
}
