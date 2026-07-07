use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

use gix::{
    credentials::helper::Action,
    progress::Discard,
    protocol::transport::client::blocking_io::Transport,
    remote::{Connection, fetch::Tags},
    sec::identity::Account,
};
use human_errors::ResultExt;
use tracing_batteries::prelude::*;

use crate::{
    BackupEntity,
    entities::{Credentials, GitRepo, RecoveryMode},
};

use super::BackupState;

/// How old a git lock file must be before it is considered stale and safe to
/// remove during automatic recovery. Healthy git operations only hold their
/// locks for a fraction of a second, so anything older than this was almost
/// certainly left behind by a process which was killed before it could clean
/// up after itself.
const STALE_LOCK_MAX_AGE: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Default)]
pub struct GitEngine;

impl GitEngine {
    #[allow(clippy::blocks_in_conditions)]
    #[tracing::instrument(skip(self, target, entity), ret, err, fields(entity = %entity))]
    pub async fn backup(
        &self,
        entity: &GitRepo,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        let target_path = target.join(entity.target_path());
        self.ensure_directory(&target_path)?;

        if target_path.join(".git").exists() {
            trace!(
                "Git directory exists at {}/.git, using fetch mode.",
                target_path.display()
            );
            match self.fetch(entity, &target_path, cancel) {
                Ok(state) => Ok(state),
                Err(error) => self.recover(entity, &target_path, cancel, error),
            }
        } else {
            trace!(
                "No Git directory found at {}/.git, using clone mode.",
                target_path.display()
            );
            self.clone(entity, &target_path, cancel)
        }
    }
}

impl GitEngine {
    fn ensure_directory(&self, path: &Path) -> Result<(), human_errors::Error> {
        trace!("Ensuring directory exists: {}", path.display());
        std::fs::create_dir_all(path).wrap_user_err(
            format!("Unable to create backup directory '{}'", path.display()),
            &["Make sure that you have permission to create the directory."],
        )
    }

    #[tracing::instrument(skip(self, repo, target, cancel), err)]
    fn clone(
        &self,
        repo: &GitRepo,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, human_errors::Error> {
        trace!(
            "Cloning repository {} into {}",
            repo.clone_url,
            target.display()
        );
        let mut fetch = gix::prepare_clone(repo.clone_url.as_str(), target).wrap_system_err(
            format!("Failed to clone the repository {}.", &repo.clone_url),
            &["Please make sure that the target directory is writable and that the repository is accessible."],
        )?;

        match &repo.credentials {
            Credentials::None => {}
            creds => {
                let creds = creds.clone();
                fetch = fetch.configure_connection(move |c| {
                    Self::authenticate_connection(c, &creds);
                    Ok(())
                });
            }
        }

        trace!("Running clone in bare mode (not checking out files)");
        let (repository, _outcome) = fetch.fetch_only(Discard, cancel).wrap_system_err(
            format!("Unable to clone remote repository '{}'", repo.clone_url),
            &["Make sure that your internet connectivity is working correctly, and that your local git configuration is able to clone this repo."],
        )?;

        trace!("Configure fallback committer information");
        self.ensure_committer(&repository)?;

        trace!("Configuring core.bare for Git repository");
        self.update_config(&repository, |c| {
            c.set_raw_value(gix::config::tree::Core::BARE, "true").wrap_system_err(
                format!("Unable to set the 'core.bare' configuration option for repository '{}'", repo.name()),
                &["Make sure that the git repository has been correctly initialized and run `git config core.bare true` to configure it correctly."],
            )?;

            Ok(())
        })?;

        let head_id = repository.head_id().wrap_user_err(
            format!("The repository '{}' did not have a valid HEAD, which may indicate that there is something wrong with the source repository.", &repo.clone_url),
            &["Make sure that the remote repository is valid."],
        )?;

        Ok(BackupState::New(Some(format!("at {}", head_id.to_hex()))))
    }

    #[tracing::instrument(skip(self, repo, target, cancel), err)]
    fn fetch(
        &self,
        repo: &GitRepo,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, human_errors::Error> {
        trace!("Opening repository {}", target.display());
        {
            let repository = gix::open(target).wrap_user_err(
                format!(
                    "Failed to open the repository '{}' at '{}'",
                    &repo.clone_url,
                    &target.display()
                ),
                &["Make sure that the target directory is a valid git repository."],
            )?;
            self.ensure_committer(&repository)?;
        }

        // Re-open the repository to pick up any config changes made by ensure_committer
        // (e.g. the gitoxide committer fallback written to the on-disk config).
        // The in-memory config of a gix::Repository is loaded at open time, so
        // writes made via update_config are not visible until the repo is reopened.
        let repository = gix::open(target).wrap_user_err(
            format!(
                "Failed to open the repository '{}' at '{}'",
                &repo.clone_url,
                &target.display()
            ),
            &["Make sure that the target directory is a valid git repository."],
        )?;

        let original_head = repository.head_id().ok();

        let default_refspecs = vec!["+refs/heads/*:refs/remotes/origin/*".to_string()];

        trace!(
            "Configuring fetch operation for repository {}",
            target.display()
        );
        let remote = repository.find_fetch_remote(Some(repo.clone_url.as_str().into())).wrap_user_err(
            format!(
                "Failed to find the remote '{}' in the repository '{}'",
                repo.clone_url,
                &target.display()
                ),
            &["Make sure that the repository is correctly configured and that the remote exists."],
        )?
            .with_fetch_tags(Tags::All)
            .with_refspecs(
              repo.refspecs.as_ref().unwrap_or(&default_refspecs)
                .iter()
                .map(|s| gix::bstr::BString::from(s.as_str()))
                .collect::<Vec<gix::bstr::BString>>(),
              gix::remote::Direction::Fetch)
            .wrap_user_err(
                format!(
                    "Failed to configure the remote '{}' in the repository '{}' to fetch all branches.",
                    &repo.clone_url,
                    &target.display()
                    ),
                    &["Make sure that the repository is correctly configured and that the remote exists."],
                )?;

        trace!("Connecting to remote repository {}", repo.clone_url);
        let mut connection = remote
            .connect(gix::remote::Direction::Fetch)
            .wrap_user_err(
                format!(
                    "Unable to establish connection to remote git repository '{}'",
                    &repo.clone_url
                ),
                &["Make sure that the repository is available and correctly configured."],
            )?;

        Self::authenticate_connection(&mut connection, &repo.credentials);

        trace!(
            "Running fetch operation for remote repository {}",
            repo.clone_url
        );
        connection
            .prepare_fetch(Discard, Default::default())
            .wrap_user_err(
                format!(
                    "Unable to prepare fetch from remote git repository '{}'",
                    &repo.clone_url
                ),
                &["Make sure that the repository is available and correctly configured."],
            )?
            .with_write_packed_refs_only(true)
            .receive(Discard, cancel)
            .wrap_user_err(
                format!(
                    "Unable to fetch from remote git repository '{}'",
                    &repo.clone_url
                ),
                &["Make sure that the repository is available and correctly configured."],
            )?;

        let head_id = repository.head_id().wrap_user_err(
            format!("The repository '{}' did not have a valid HEAD, which may indicate that there is something wrong with the source repository.", &repo.clone_url),
            &["Make sure that the remote repository is valid."],
        )?;

        if let Some(original_head) = original_head
            && original_head == head_id
        {
            return Ok(BackupState::Unchanged(Some(format!(
                "at {}",
                head_id.to_hex()
            ))));
        }

        Ok(BackupState::Updated(Some(format!("{}", head_id.to_hex()))))
    }

    /// Attempts to automatically recover from a failed update of an existing
    /// local repository, according to the entity's configured [`RecoveryMode`].
    ///
    /// Recovery is staged from least to most invasive: first stale git lock
    /// files (left behind by a previous run which was killed part-way through
    /// a fetch) are removed and the fetch is retried; if that doesn't resolve
    /// the problem and destructive recovery has been enabled, a fresh copy of
    /// the repository is cloned into a temporary location and swapped into
    /// place. The original error is reported if recovery isn't possible.
    #[tracing::instrument(skip(self, repo, target, cancel, error), err, fields(mode = %repo.recovery_mode))]
    fn recover(
        &self,
        repo: &GitRepo,
        target: &Path,
        cancel: &AtomicBool,
        error: human_errors::Error,
    ) -> Result<BackupState, human_errors::Error> {
        if repo.recovery_mode == RecoveryMode::Disabled
            || cancel.load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(error);
        }

        warn!(
            "Updating the local copy of '{}' failed, attempting automatic recovery: {}",
            repo.name(),
            error
        );

        let error = match self.remove_stale_locks(target, STALE_LOCK_MAX_AGE) {
            Ok(removed) if !removed.is_empty() => {
                warn!(
                    "Removed {} stale git lock file(s) from '{}', retrying the fetch.",
                    removed.len(),
                    target.display()
                );

                match self.fetch(repo, target, cancel) {
                    Ok(state) => {
                        return Ok(annotate_state(
                            state,
                            "recovered by removing stale lock files",
                        ));
                    }
                    Err(retry_error) => retry_error,
                }
            }
            Ok(_) => error,
            Err(cleanup_error) => {
                warn!(
                    "Unable to clean up stale git lock files in '{}': {}",
                    target.display(),
                    cleanup_error
                );
                error
            }
        };

        if repo.recovery_mode != RecoveryMode::Destructive {
            return Err(error);
        }

        warn!(
            "Attempting destructive recovery of '{}' by cloning a fresh copy of '{}'.",
            target.display(),
            repo.clone_url
        );

        match self.reclone_and_replace(repo, target, cancel) {
            Ok(state) => Ok(annotate_state(
                state,
                "recovered by re-cloning the repository",
            )),
            Err(recovery_error) => Err(human_errors::wrap_user(
                error,
                format!(
                    "We also attempted to automatically recover the local repository at '{}' by re-cloning it, however this failed too: {}",
                    target.display(),
                    recovery_error
                ),
                &[
                    "Make sure that the remote repository is accessible and that the backup target is writable.",
                    "If the problem persists, you may need to remove the local copy of the repository manually so that it can be re-cloned from scratch.",
                ],
            )),
        }
    }

    /// Removes any git lock files within the repository's `.git` directory
    /// which are older than `max_age`, returning the paths of the files which
    /// were removed.
    ///
    /// Git (and gitoxide) take out `*.lock` files next to the file they intend
    /// to replace and rename them into place once the update is complete. When
    /// a process is killed part-way through an update these lock files are left
    /// behind and block every subsequent update. The age threshold ensures that
    /// we only ever remove locks which cannot still be held by a live git
    /// operation.
    fn remove_stale_locks(
        &self,
        target: &Path,
        max_age: Duration,
    ) -> Result<Vec<PathBuf>, human_errors::Error> {
        let git_dir = target.join(".git");
        if !git_dir.is_dir() {
            return Ok(Vec::new());
        }

        let now = std::time::SystemTime::now();
        let mut removed = Vec::new();
        let mut pending = vec![git_dir];

        while let Some(dir) = pending.pop() {
            let entries = std::fs::read_dir(&dir).wrap_system_err(
                format!(
                    "Unable to enumerate the directory '{}' while looking for stale git lock files.",
                    dir.display()
                ),
                &["Make sure that the backup directory is readable by the backup process."],
            )?;

            for entry in entries {
                let entry = entry.wrap_system_err(
                    format!(
                        "Unable to enumerate the directory '{}' while looking for stale git lock files.",
                        dir.display()
                    ),
                    &["Make sure that the backup directory is readable by the backup process."],
                )?;

                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };

                if file_type.is_dir() {
                    pending.push(path);
                } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "lock") {
                    // Treat any lock whose age we cannot determine as fresh so
                    // that we never remove a lock which might still be held.
                    let stale = entry
                        .metadata()
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| now.duration_since(modified).ok())
                        .is_some_and(|age| age >= max_age);

                    if stale {
                        match std::fs::remove_file(&path) {
                            Ok(()) => removed.push(path),
                            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                            Err(err) => {
                                return Err(human_errors::wrap_system(
                                    err,
                                    format!(
                                        "Unable to remove the stale git lock file '{}'.",
                                        path.display()
                                    ),
                                    &[
                                        "Make sure that the backup directory is writable by the backup process.",
                                    ],
                                ));
                            }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Clones a fresh copy of the repository into a temporary directory
    /// alongside the current one and, if the clone succeeds, replaces the
    /// (presumed corrupt) local repository with the fresh clone.
    ///
    /// The clone acting as a canary is what makes this safe to attempt on any
    /// failure: if the remote is unreachable, the credentials are wrong, or the
    /// disk is full, the clone fails and the original repository is left
    /// untouched. The local copy is only ever replaced by a clone which we know
    /// to be complete and healthy.
    fn reclone_and_replace(
        &self,
        repo: &GitRepo,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, human_errors::Error> {
        let (parent, dir_name) = match (
            target.parent(),
            target.file_name().and_then(|n| n.to_str()),
        ) {
            (Some(parent), Some(dir_name)) => (parent, dir_name),
            _ => {
                return Err(human_errors::system(
                    format!(
                        "Unable to determine a temporary recovery location for the backup target '{}'.",
                        target.display()
                    ),
                    &["Please report this issue to us on GitHub."],
                ));
            }
        };

        // Both directories live alongside the target so that the renames below
        // remain on the same filesystem (and therefore atomic).
        let staging = parent.join(format!(".{dir_name}.recovery"));
        let discard = parent.join(format!(".{dir_name}.discard"));

        for dir in [&staging, &discard] {
            if dir.exists() {
                std::fs::remove_dir_all(dir).wrap_system_err(
                    format!(
                        "Unable to remove the left-over recovery directory '{}'.",
                        dir.display()
                    ),
                    &["Make sure that the backup directory is writable by the backup process."],
                )?;
            }
        }

        let state = self.clone(repo, &staging, cancel)?;

        // The fresh clone succeeded, so the failure was local to our copy of
        // the repository and it is safe to replace it.
        std::fs::rename(target, &discard).wrap_system_err(
            format!(
                "Unable to move the corrupt repository '{}' out of the way during recovery.",
                target.display()
            ),
            &["Make sure that the backup directory is writable by the backup process."],
        )?;

        if let Err(err) = std::fs::rename(&staging, target) {
            // Try to put the original repository back so that we don't lose
            // whatever data it still holds.
            let _ = std::fs::rename(&discard, target);
            let _ = std::fs::remove_dir_all(&staging);

            return Err(human_errors::wrap_system(
                err,
                format!(
                    "Unable to move the freshly cloned repository into place at '{}' during recovery.",
                    target.display()
                ),
                &["Make sure that the backup directory is writable by the backup process."],
            ));
        }

        if let Err(err) = std::fs::remove_dir_all(&discard) {
            warn!(
                "Unable to remove the old copy of the repository at '{}' after recovery, you may wish to remove it manually: {}",
                discard.display(),
                err
            );
        }

        Ok(state)
    }

    fn authenticate_connection<T: Transport>(
        connection: &mut Connection<'_, '_, '_, T>,
        creds: &Credentials,
    ) {
        match creds {
            Credentials::None => {}
            creds => {
                trace!("Configuring credentials for Git connection");
                let creds = creds.clone();
                #[allow(clippy::result_large_err)]
                connection.set_credentials(move |a| match a {
                    Action::Get(ctx) => Ok(Some(gix::credentials::protocol::Outcome {
                        identity: match &creds {
                            Credentials::None => Account {
                                username: "".into(),
                                password: "".into(),
                                oauth_refresh_token: None,
                            },
                            Credentials::Token(token) => Account {
                                username: token.clone(),
                                password: "".into(),
                                oauth_refresh_token: None,
                            },
                            Credentials::UsernamePassword { username, password } => Account {
                                username: username.clone(),
                                password: password.clone(),
                                oauth_refresh_token: None,
                            },
                        },
                        next: ctx.into(),
                    })),
                    _ => Ok(None),
                });
            }
        }
    }

    fn ensure_committer(&self, repo: &gix::Repository) -> Result<(), human_errors::Error> {
        if repo.committer().is_none() {
            self.update_config(repo, |cfg| {
                cfg.set_raw_value(
                    gix::config::tree::gitoxide::Committer::NAME_FALLBACK,
                    "github-backup",
                )
                .expect("works - statically known");
                cfg.set_raw_value(
                    gix::config::tree::gitoxide::Committer::EMAIL_FALLBACK,
                    "github-backup@sierrasoftworks.github.io",
                )
                .expect("works - statically known");

                Ok(())
            })
        } else {
            Ok(())
        }
    }

    fn update_config<U>(
        &self,
        repo: &gix::Repository,
        mut update: U,
    ) -> Result<(), human_errors::Error>
    where
        U: FnMut(&mut gix::config::File<'_>) -> Result<(), human_errors::Error>,
    {
        let mut config = gix::config::File::from_path_no_includes(
            repo.path().join("config"),
            gix::config::Source::Local,
        )
        .wrap_system_err(
            format!(
                "Unable to load git configuration for repository '{}'",
                repo.path().display()
            ),
            &["Make sure that the git repository has been correctly initialized."],
        )?;

        update(&mut config)?;

        let mut file = std::fs::File::create(repo.path().join("config")).wrap_system_err(
            format!(
                "Unable to write git configuration for repository '{}'",
                repo.path().display()
            ),
            &["Make sure that the git repository has been correctly initialized."],
        )?;

        config.write_to(&mut file).wrap_system_err(
            format!(
                "Unable to write git configuration for repository '{}'",
                repo.path().display()
            ),
            &["Make sure that the git repository has been correctly initialized."],
        )
    }
}

impl Display for GitEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "git")
    }
}

/// Appends a note to a [`BackupState`]'s description so that backups which
/// were only completed through automatic recovery are visible as such in the
/// backup summary.
fn annotate_state(state: BackupState, note: &str) -> BackupState {
    let annotate = |detail: Option<String>| {
        Some(match detail {
            Some(detail) => format!("{detail}; {note}"),
            None => note.to_string(),
        })
    };

    match state {
        BackupState::New(detail) => BackupState::New(annotate(detail)),
        BackupState::Updated(detail) => BackupState::Updated(annotate(detail)),
        BackupState::Unchanged(detail) => BackupState::Unchanged(annotate(detail)),
        BackupState::Skipped => BackupState::Skipped,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[cfg_attr(feature = "pure_tests", ignore)]
    #[rstest]
    #[case("SierraSoftworks/grey", "https://github.com/sierrasoftworks/grey.git")]
    #[tokio::test]
    async fn test_backup(#[case] name: &str, #[case] url: &str) {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let agent = GitEngine;
        let cancel = AtomicBool::new(false);

        let repo = GitRepo::new(name, url, None);

        let state1 = agent
            .backup(&repo, temp_dir.path(), &cancel)
            .await
            .expect("initial backup to succeed (clone)");
        assert!(
            temp_dir
                .path()
                .join(repo.target_path())
                .join(".git")
                .exists(),
            "the repository should have been created"
        );

        assert!(
            matches!(state1, BackupState::New(..)),
            "the repository should have been cloned initially"
        );

        let state2 = agent
            .backup(&repo, temp_dir.path(), &cancel)
            .await
            .expect("subsequent backup to succeed (fetch)");

        assert!(
            matches!(state2, BackupState::Unchanged(..)),
            "the repository should not have changed between backups"
        );
    }

    #[test]
    fn test_remove_stale_locks() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");
        let git_dir = temp_dir.path().join(".git");
        std::fs::create_dir_all(git_dir.join("refs").join("heads"))
            .expect("to be able to create the git directory structure");

        let stale_locks = [
            git_dir.join("packed-refs.lock"),
            git_dir.join("refs").join("heads").join("main.lock"),
        ];
        let fresh_lock = git_dir.join("HEAD.lock");
        let config_file = git_dir.join("config");

        for path in stale_locks.iter().chain([&fresh_lock, &config_file]) {
            std::fs::write(path, "contents").expect("to be able to create the test file");
        }

        let stale_time = std::time::SystemTime::now() - Duration::from_secs(60 * 60);
        for path in &stale_locks {
            std::fs::File::options()
                .write(true)
                .open(path)
                .expect("to be able to open the lock file")
                .set_modified(stale_time)
                .expect("to be able to age the lock file");
        }

        let removed = GitEngine
            .remove_stale_locks(temp_dir.path(), STALE_LOCK_MAX_AGE)
            .expect("lock cleanup to succeed");

        assert_eq!(
            removed.len(),
            2,
            "only the stale lock files should have been removed"
        );
        for path in &stale_locks {
            assert!(
                !path.exists(),
                "the stale lock file '{}' should have been removed",
                path.display()
            );
            assert!(removed.contains(path));
        }

        assert!(
            fresh_lock.exists(),
            "recently created lock files should be left in place"
        );
        assert!(
            config_file.exists(),
            "files which are not lock files should be left in place"
        );
    }

    #[test]
    fn test_remove_stale_locks_without_git_dir() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let removed = GitEngine
            .remove_stale_locks(temp_dir.path(), Duration::ZERO)
            .expect("lock cleanup to succeed");

        assert!(
            removed.is_empty(),
            "no locks should be removed when there is no .git directory"
        );
    }

    #[rstest]
    #[case(BackupState::New(Some("at abc123".into())), "new at abc123; recovered")]
    #[case(BackupState::Updated(None), "updated recovered")]
    #[case(BackupState::Unchanged(Some("at abc123".into())), "unchanged at abc123; recovered")]
    #[case(BackupState::Skipped, "skipped")]
    fn test_annotate_state(#[case] state: BackupState, #[case] expected: &str) {
        assert_eq!(format!("{}", annotate_state(state, "recovered")), expected);
    }

    #[cfg_attr(feature = "pure_tests", ignore)]
    #[rstest]
    #[case("SierraSoftworks/grey", "https://github.com/sierrasoftworks/grey.git")]
    #[tokio::test]
    async fn test_stale_lock_recovery(#[case] name: &str, #[case] url: &str) {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let agent = GitEngine;
        let cancel = AtomicBool::new(false);

        let repo = GitRepo::new(name, url, None);

        agent
            .backup(&repo, temp_dir.path(), &cancel)
            .await
            .expect("initial backup to succeed (clone)");

        // Simulate a lock file which was left behind by a previous run that
        // was killed part-way through a fetch operation.
        let repo_path = temp_dir.path().join(repo.target_path());
        let lock_file = repo_path.join(".git").join("packed-refs.lock");
        std::fs::write(&lock_file, "").expect("to be able to create the lock file");
        std::fs::File::options()
            .write(true)
            .open(&lock_file)
            .expect("to be able to open the lock file")
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(60 * 60))
            .expect("to be able to age the lock file");

        let state = agent
            .recover(
                &repo,
                &repo_path,
                &cancel,
                human_errors::user("simulated fetch failure", &[]),
            )
            .expect("recovery to succeed after removing the stale lock file");

        assert!(
            !lock_file.exists(),
            "the stale lock file should have been removed"
        );
        assert!(
            matches!(state, BackupState::Unchanged(..)),
            "the retried fetch should report the repository as unchanged"
        );
    }

    #[cfg_attr(feature = "pure_tests", ignore)]
    #[rstest]
    #[case("SierraSoftworks/grey", "https://github.com/sierrasoftworks/grey.git")]
    #[tokio::test]
    async fn test_destructive_recovery(#[case] name: &str, #[case] url: &str) {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let agent = GitEngine;
        let cancel = AtomicBool::new(false);

        let repo = GitRepo::new(name, url, None);

        agent
            .backup(&repo, temp_dir.path(), &cancel)
            .await
            .expect("initial backup to succeed (clone)");

        // Corrupt the repository in a way which cannot be repaired
        // non-destructively.
        let repo_path = temp_dir.path().join(repo.target_path());
        let config_file = repo_path.join(".git").join("config");
        let corrupt_config = "this is not [a valid git config";
        std::fs::write(&config_file, corrupt_config).expect("to be able to corrupt the config");

        agent
            .backup(&repo, temp_dir.path(), &cancel)
            .await
            .expect_err("the backup should fail when the repository is corrupted");

        let repo = repo.with_recovery_mode(RecoveryMode::Destructive);
        let state = agent
            .backup(&repo, temp_dir.path(), &cancel)
            .await
            .expect("destructive recovery to replace the corrupted repository");

        assert!(
            matches!(state, BackupState::New(..)),
            "the repository should have been re-cloned"
        );
        assert!(
            repo_path.join(".git").exists(),
            "the recovered repository should exist at the original location"
        );
        assert_ne!(
            std::fs::read_to_string(&config_file).expect("to be able to read the config"),
            corrupt_config,
            "the corrupted config should have been replaced"
        );

        let parent = repo_path.parent().expect("the repository to have a parent");
        let dir_name = repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("the repository to have a directory name");
        assert!(
            !parent.join(format!(".{dir_name}.recovery")).exists(),
            "the recovery staging directory should have been cleaned up"
        );
        assert!(
            !parent.join(format!(".{dir_name}.discard")).exists(),
            "the discarded repository should have been cleaned up"
        );

        // A subsequent backup of the recovered repository should work normally.
        let state = agent
            .backup(&repo, temp_dir.path(), &cancel)
            .await
            .expect("a subsequent backup to succeed (fetch)");
        assert!(
            matches!(state, BackupState::Unchanged(..)),
            "the recovered repository should be fetchable"
        );
    }
}
