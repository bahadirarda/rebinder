use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Output},
};

use serde::Serialize;
use thiserror::Error;

const MAX_SIBLING_REPOSITORIES: usize = 128;

/// Explicit policy for rebuilding a missing workspace from Git's worktree registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum WorktreeRecovery {
    /// Never create a workspace path.
    #[default]
    Disabled,
    /// Recreate only a still-registered, unlocked worktree. An optional repository
    /// removes discovery ambiguity.
    Registered { repository: Option<PathBuf> },
}

impl WorktreeRecovery {
    pub fn registered(repository: Option<PathBuf>) -> Self {
        Self::Registered { repository }
    }
}

/// Verified facts about a worktree recreated by Rebinder.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveredWorktree {
    pub path: PathBuf,
    pub repository: PathBuf,
    pub head: String,
    pub branch: Option<String>,
}

/// Fail-closed recovery errors. Rebinder never clones or overwrites a path.
#[derive(Debug, Error)]
pub enum WorktreeRecoveryError {
    #[error("worktree recovery target must be absolute: `{0}`")]
    RelativeTarget(PathBuf),
    #[error("worktree recovery target already exists and will not be overwritten: `{0}`")]
    TargetExists(PathBuf),
    #[error("cannot inspect worktree recovery target `{target}`: {source}")]
    InspectTarget {
        target: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot reserve missing worktree path `{target}` without overwriting it: {source}")]
    ReserveTarget {
        target: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("worktree recovery target has no existing parent directory: `{0}`")]
    MissingParent(PathBuf),
    #[error(
        "worktree recovery parent traverses a symbolic link and is unsafe to create beneath: `{0}`"
    )]
    SymlinkedParent(PathBuf),
    #[error("worktree repository is not an existing directory: `{0}`")]
    InvalidRepository(PathBuf),
    #[error(
        "automatic repository discovery found more than {maximum} sibling Git directories under `{parent}`; pass `--worktree-repository REPOSITORY`"
    )]
    DiscoveryTooBroad { parent: PathBuf, maximum: usize },
    #[error("cannot inspect candidate repository `{repository}`: {source}")]
    InspectRepository {
        repository: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Git rejected repository inspection for `{repository}`: {message}")]
    GitInspection {
        repository: PathBuf,
        message: String,
    },
    #[error("Git returned non-UTF-8 registry data for `{repository}`; recovery stopped")]
    GitOutputEncoding { repository: PathBuf },
    #[error(
        "no Git repository still registers missing worktree `{target}`; pass `--worktree-repository REPOSITORY` if its main worktree is outside the recorded path"
    )]
    RegistrationNotFound { target: PathBuf },
    #[error("multiple repositories register missing worktree `{target}`: {repositories:?}")]
    AmbiguousRegistration {
        target: PathBuf,
        repositories: Vec<PathBuf>,
    },
    #[error(
        "registered worktree `{target}` is locked{reason}; unlock it explicitly before recovery"
    )]
    Locked { target: PathBuf, reason: String },
    #[error("registered worktree `{target}` has no valid recorded commit")]
    MissingHead { target: PathBuf },
    #[error("registered branch `{branch}` no longer resolves to recorded commit `{head}`")]
    BranchMoved { branch: String, head: String },
    #[error("cannot recreate registered worktree `{target}` from `{repository}`: {message}")]
    GitAdd {
        target: PathBuf,
        repository: PathBuf,
        message: String,
    },
    #[error(
        "Git created `{target}` but its verified HEAD or common repository did not match the registry"
    )]
    VerificationFailed { target: PathBuf },
}

#[derive(Debug, Clone)]
struct RegisteredWorktree {
    path: PathBuf,
    head: String,
    branch: Option<String>,
    locked: Option<String>,
}

#[derive(Debug, Clone)]
struct RecoveryCandidate {
    repository: PathBuf,
    common_directory: PathBuf,
    registered: RegisteredWorktree,
}

/// Recreate a missing worktree only when Git itself still has an exact registry entry.
pub fn recover_registered_worktree(
    target: &Path,
    repository_hint: Option<&Path>,
) -> Result<RecoveredWorktree, WorktreeRecoveryError> {
    let target = normalized_absolute(target)?;
    let target = validate_target_path(&target)?;
    let repositories = candidate_repositories(&target, repository_hint)?;
    let mut matches = Vec::new();
    for repository in repositories {
        let common_directory = git_common_directory(&repository)?;
        let entries = registered_worktrees(&repository)?;
        if let Some(registered) = entries
            .into_iter()
            .find(|entry| normalized_absolute(&entry.path).is_ok_and(|path| path == target))
        {
            matches.push(RecoveryCandidate {
                repository,
                common_directory,
                registered,
            });
        }
    }

    let candidate = match matches.len() {
        0 => return Err(WorktreeRecoveryError::RegistrationNotFound { target }),
        1 => matches.remove(0),
        _ => {
            return Err(WorktreeRecoveryError::AmbiguousRegistration {
                target,
                repositories: matches
                    .into_iter()
                    .map(|candidate| candidate.repository)
                    .collect(),
            });
        }
    };
    if let Some(reason) = candidate.registered.locked.as_deref() {
        let reason = if reason.is_empty() {
            String::new()
        } else {
            format!(" ({reason})")
        };
        return Err(WorktreeRecoveryError::Locked {
            target: candidate.registered.path,
            reason,
        });
    }
    validate_commit(&candidate.repository, &candidate.registered.head, &target)?;
    if let Some(branch) = candidate.registered.branch.as_deref() {
        let current = git_text(
            &candidate.repository,
            ["rev-parse", "--verify", &format!("{branch}^{{commit}}")],
        )?;
        if current != candidate.registered.head {
            return Err(WorktreeRecoveryError::BranchMoved {
                branch: branch.to_owned(),
                head: candidate.registered.head,
            });
        }
    }

    fs::create_dir(&target).map_err(|source| WorktreeRecoveryError::ReserveTarget {
        target: target.clone(),
        source,
    })?;

    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(&candidate.repository)
        .args(["worktree", "add", "--force"]);
    if candidate.registered.branch.is_none() {
        command.arg("--detach");
    }
    command.arg("--").arg(&target);
    let checkout = candidate
        .registered
        .branch
        .as_deref()
        .and_then(|branch| branch.strip_prefix("refs/heads/"))
        .unwrap_or(&candidate.registered.head);
    command.arg(checkout);
    let output = match command.output() {
        Ok(output) => output,
        Err(source) => {
            let _ = fs::remove_dir(&target);
            return Err(WorktreeRecoveryError::InspectRepository {
                repository: candidate.repository.clone(),
                source,
            });
        }
    };
    if !output.status.success() {
        let _ = fs::remove_dir(&target);
        return Err(WorktreeRecoveryError::GitAdd {
            target,
            repository: candidate.repository,
            message: output_message(&output),
        });
    }

    let recovered_head = git_text(&target, ["rev-parse", "--verify", "HEAD^{commit}"])?;
    let recovered_common = git_common_directory(&target)?;
    let recovered_branch_matches = candidate.registered.branch.as_deref().is_none_or(|branch| {
        git_text(&target, ["symbolic-ref", "--quiet", "HEAD"])
            .is_ok_and(|recovered| recovered == branch)
    });
    if recovered_head != candidate.registered.head
        || recovered_common != candidate.common_directory
        || !recovered_branch_matches
        || !target.is_dir()
    {
        return Err(WorktreeRecoveryError::VerificationFailed { target });
    }

    Ok(RecoveredWorktree {
        path: target,
        repository: candidate.repository,
        head: candidate.registered.head,
        branch: candidate.registered.branch,
    })
}

fn normalized_absolute(path: &Path) -> Result<PathBuf, WorktreeRecoveryError> {
    if !path.is_absolute() {
        return Err(WorktreeRecoveryError::RelativeTarget(path.to_path_buf()));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(WorktreeRecoveryError::RelativeTarget(path.to_path_buf()));
                }
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    Ok(normalized)
}

fn validate_target_path(target: &Path) -> Result<PathBuf, WorktreeRecoveryError> {
    match fs::symlink_metadata(target) {
        Ok(_) => return Err(WorktreeRecoveryError::TargetExists(target.to_path_buf())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(WorktreeRecoveryError::InspectTarget {
                target: target.to_path_buf(),
                source,
            });
        }
    }
    let parent = target
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or_else(|| WorktreeRecoveryError::MissingParent(target.to_path_buf()))?;
    if fs::symlink_metadata(parent).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(WorktreeRecoveryError::SymlinkedParent(parent.to_path_buf()));
    }
    let canonical_parent = parent
        .canonicalize()
        .map_err(|_| WorktreeRecoveryError::MissingParent(parent.to_path_buf()))?;
    let name = target
        .file_name()
        .ok_or_else(|| WorktreeRecoveryError::MissingParent(target.to_path_buf()))?;
    Ok(canonical_parent.join(name))
}

fn candidate_repositories(
    target: &Path,
    repository_hint: Option<&Path>,
) -> Result<Vec<PathBuf>, WorktreeRecoveryError> {
    if let Some(repository) = repository_hint {
        return Ok(vec![canonical_repository(repository)?]);
    }

    let mut repositories = BTreeSet::new();
    for ancestor in target.ancestors().skip(1).filter(|path| path.is_dir()) {
        if let Ok(repository) = canonical_repository(ancestor) {
            repositories.insert(repository);
        }
    }
    if !repositories.is_empty() {
        return Ok(repositories.into_iter().collect());
    }

    let Some(parent) = target.parent().filter(|path| path.is_dir()) else {
        return Ok(Vec::new());
    };
    let entries =
        fs::read_dir(parent).map_err(|source| WorktreeRecoveryError::InspectRepository {
            repository: parent.to_path_buf(),
            source,
        })?;
    let mut sibling_repositories = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path != target && path.join(".git").exists())
        .collect::<Vec<_>>();
    sibling_repositories.sort();
    if sibling_repositories.len() > MAX_SIBLING_REPOSITORIES {
        return Err(WorktreeRecoveryError::DiscoveryTooBroad {
            parent: parent.to_path_buf(),
            maximum: MAX_SIBLING_REPOSITORIES,
        });
    }
    for path in sibling_repositories {
        if let Ok(repository) = canonical_repository(&path) {
            repositories.insert(repository);
        }
    }
    Ok(repositories.into_iter().collect())
}

fn canonical_repository(path: &Path) -> Result<PathBuf, WorktreeRecoveryError> {
    if !path.is_dir() {
        return Err(WorktreeRecoveryError::InvalidRepository(path.to_path_buf()));
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| WorktreeRecoveryError::InvalidRepository(path.to_path_buf()))?;
    let top_level = git_text(&canonical, ["rev-parse", "--show-toplevel"])?;
    PathBuf::from(top_level)
        .canonicalize()
        .map_err(|_| WorktreeRecoveryError::InvalidRepository(path.to_path_buf()))
}

fn registered_worktrees(
    repository: &Path,
) -> Result<Vec<RegisteredWorktree>, WorktreeRecoveryError> {
    let output = git_output(repository, ["worktree", "list", "--porcelain", "-z"])?;
    if !output.status.success() {
        return Err(WorktreeRecoveryError::GitInspection {
            repository: repository.to_path_buf(),
            message: output_message(&output),
        });
    }
    let text =
        String::from_utf8(output.stdout).map_err(|_| WorktreeRecoveryError::GitOutputEncoding {
            repository: repository.to_path_buf(),
        })?;
    let mut entries = Vec::new();
    let mut current: Option<RegisteredWorktree> = None;
    for field in text.split('\0').chain(std::iter::once("")) {
        if field.is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }
        if let Some(path) = field.strip_prefix("worktree ") {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            current = Some(RegisteredWorktree {
                path: PathBuf::from(path),
                head: String::new(),
                branch: None,
                locked: None,
            });
        } else if let Some(entry) = current.as_mut() {
            if let Some(head) = field.strip_prefix("HEAD ") {
                head.clone_into(&mut entry.head);
            } else if let Some(branch) = field.strip_prefix("branch ") {
                entry.branch = Some(branch.to_owned());
            } else if field == "locked" {
                entry.locked = Some(String::new());
            } else if let Some(reason) = field.strip_prefix("locked ") {
                entry.locked = Some(reason.to_owned());
            }
        }
    }
    Ok(entries)
}

fn validate_commit(
    repository: &Path,
    head: &str,
    target: &Path,
) -> Result<(), WorktreeRecoveryError> {
    if head.is_empty() || !head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(WorktreeRecoveryError::MissingHead {
            target: target.to_path_buf(),
        });
    }
    let output = git_output(
        repository,
        ["cat-file", "-e", &format!("{head}^{{commit}}")],
    )?;
    if !output.status.success() {
        return Err(WorktreeRecoveryError::MissingHead {
            target: target.to_path_buf(),
        });
    }
    Ok(())
}

fn git_common_directory(repository: &Path) -> Result<PathBuf, WorktreeRecoveryError> {
    let value = git_text(repository, ["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(value);
    let resolved = if path.is_absolute() {
        path
    } else {
        repository.join(path)
    };
    resolved
        .canonicalize()
        .map_err(|_| WorktreeRecoveryError::InvalidRepository(repository.to_path_buf()))
}

fn git_text<I, S>(repository: &Path, arguments: I) -> Result<String, WorktreeRecoveryError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = git_output(repository, arguments)?;
    if !output.status.success() {
        return Err(WorktreeRecoveryError::GitInspection {
            repository: repository.to_path_buf(),
            message: output_message(&output),
        });
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|_| WorktreeRecoveryError::GitOutputEncoding {
            repository: repository.to_path_buf(),
        })
}

fn git_output<I, S>(repository: &Path, arguments: I) -> Result<Output, WorktreeRecoveryError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|source| WorktreeRecoveryError::InspectRepository {
            repository: repository.to_path_buf(),
            source,
        })
}

fn output_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git(repository: &Path, arguments: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .output()
            .expect("run Git fixture command");
        assert!(
            output.status.success(),
            "Git fixture failed: {}",
            output_message(&output)
        );
    }

    fn repository_with_missing_worktree() -> (tempfile::TempDir, PathBuf, PathBuf, String) {
        let fixture = tempfile::tempdir().expect("create fixture");
        let repository = fixture.path().join("repository");
        let target = fixture.path().join("feature");
        fs::create_dir_all(&repository).expect("create repository");
        git(&repository, &["init", "-b", "main"]);
        git(&repository, &["config", "user.name", "Rebinder Tests"]);
        git(
            &repository,
            &["config", "user.email", "rebinder-tests@example.invalid"],
        );
        fs::write(repository.join("tracked.txt"), "committed\n").expect("write tracked file");
        git(&repository, &["add", "tracked.txt"]);
        git(&repository, &["commit", "-m", "fixture"]);
        git(&repository, &["branch", "feature"]);
        let target_text = target.to_string_lossy().into_owned();
        git(&repository, &["worktree", "add", &target_text, "feature"]);
        let head = git_text(&repository, ["rev-parse", "HEAD"]).expect("fixture head");
        fs::remove_dir_all(&target).expect("simulate a missing worktree");
        (fixture, repository, target, head)
    }

    #[test]
    fn recreates_an_exact_registered_missing_worktree() {
        let (_fixture, repository, target, head) = repository_with_missing_worktree();
        let recovered = recover_registered_worktree(&target, Some(&repository)).expect("recover");
        assert_eq!(recovered.path, target);
        assert_eq!(recovered.head, head);
        assert_eq!(recovered.branch.as_deref(), Some("refs/heads/feature"));
        assert_eq!(
            fs::read_to_string(recovered.path.join("tracked.txt")).expect("read recovered file"),
            "committed\n"
        );
    }

    #[test]
    fn discovers_a_sibling_repository_without_a_hint() {
        let (_fixture, _repository, target, head) = repository_with_missing_worktree();
        let recovered = recover_registered_worktree(&target, None).expect("discover and recover");
        assert_eq!(recovered.path, target);
        assert_eq!(recovered.head, head);
    }

    #[test]
    fn refuses_an_unregistered_target() {
        let fixture = tempfile::tempdir().expect("create fixture");
        let repository = fixture.path().join("repository");
        fs::create_dir(&repository).expect("create repository");
        git(&repository, &["init", "-b", "main"]);
        let target = fixture.path().join("missing");
        let error = recover_registered_worktree(&target, Some(&repository))
            .expect_err("unregistered target must fail");
        assert!(matches!(
            error,
            WorktreeRecoveryError::RegistrationNotFound { .. }
        ));
        assert!(!target.exists());
    }

    #[test]
    fn refuses_a_locked_registered_worktree() {
        let (_fixture, repository, target, _) = repository_with_missing_worktree();
        let target_text = target.to_string_lossy().into_owned();
        git(
            &repository,
            &["worktree", "lock", "--reason", "manual hold", &target_text],
        );
        let error = recover_registered_worktree(&target, Some(&repository))
            .expect_err("locked target must fail");
        assert!(matches!(error, WorktreeRecoveryError::Locked { .. }));
        assert!(!target.exists());
    }

    #[test]
    fn refuses_to_overwrite_a_path_created_after_the_session() {
        let (_fixture, repository, target, _) = repository_with_missing_worktree();
        fs::create_dir(&target).expect("create conflicting target");
        fs::write(target.join("sentinel"), "keep\n").expect("write sentinel");
        let error = recover_registered_worktree(&target, Some(&repository))
            .expect_err("existing target must fail");
        assert!(matches!(error, WorktreeRecoveryError::TargetExists(_)));
        assert_eq!(
            fs::read_to_string(target.join("sentinel")).expect("read sentinel"),
            "keep\n"
        );
    }

    #[test]
    fn recreates_a_registered_detached_worktree() {
        let fixture = tempfile::tempdir().expect("create fixture");
        let repository = fixture.path().join("repository");
        let worktrees = fixture.path().join("worktrees");
        let target = worktrees.join("detached");
        fs::create_dir_all(&repository).expect("create repository");
        fs::create_dir_all(&worktrees).expect("create worktree parent");
        git(&repository, &["init", "-b", "main"]);
        git(&repository, &["config", "user.name", "Rebinder Tests"]);
        git(
            &repository,
            &["config", "user.email", "rebinder-tests@example.invalid"],
        );
        fs::write(repository.join("tracked.txt"), "detached state\n").expect("write tracked file");
        git(&repository, &["add", "tracked.txt"]);
        git(&repository, &["commit", "-m", "fixture"]);
        let target_text = target.to_string_lossy().into_owned();
        git(
            &repository,
            &["worktree", "add", "--detach", &target_text, "HEAD"],
        );
        let head = git_text(&repository, ["rev-parse", "HEAD"]).expect("fixture head");
        fs::remove_dir_all(&target).expect("simulate missing detached worktree");

        let recovered = recover_registered_worktree(&target, Some(&repository)).expect("recover");
        assert_eq!(recovered.head, head);
        assert_eq!(recovered.branch, None);
        assert!(
            !git_output(&target, ["symbolic-ref", "--quiet", "HEAD"])
                .expect("inspect recovered HEAD")
                .status
                .success()
        );
    }
}
