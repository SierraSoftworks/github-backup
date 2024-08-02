use std::{fmt::Display, path::Path, sync::atomic::AtomicBool};

use gix::{
    credentials::helper::Action,
    progress::Discard,
    remote::{fetch::Tags, Connection},
    sec::identity::Account,
};
use tracing::instrument;

use crate::{
    entities::{Credentials, GitRepo},
    errors, BackupEntity,
};

use super::{BackupEngine, BackupState};

#[derive(Clone)]
pub struct GitEngine;

#[async_trait::async_trait]
impl BackupEngine<GitRepo> for GitEngine {
    #[allow(clippy::blocks_in_conditions)]
    #[tracing::instrument(skip(self, target), res, err)]
    async fn backup<P: AsRef<Path> + Send>(
        &self,
        entity: &GitRepo,
        target: P,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        let target_path = target.as_ref().join(entity.target_path());
        self.ensure_directory(&target_path)?;

        if target_path.join(".git").exists() {
            self.fetch(entity, &target_path, cancel)
        } else {
            self.clone(entity, &target_path, cancel)
        }
    }
}

impl GitEngine {
    fn ensure_directory(&self, path: &Path) -> Result<(), errors::Error> {
        std::fs::create_dir_all(path).map_err(|e| {
            errors::user_with_internal(
                &format!("Unable to create backup directory '{}'", path.display()),
                "Make sure that you have permission to create the directory.",
                e,
            )
        })
    }

    #[instrument(skip(self, repo, target, cancel), err)]
    fn clone(
        &self,
        repo: &GitRepo,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, errors::Error> {
        let mut fetch = gix::prepare_clone(repo.clone_url.as_str(), target).map_err(|e| errors::system_with_internal(
            &format!("Failed to clone the repository {}.", &repo.clone_url),
            "Please make sure that the target directory is writable and that the repository is accessible.",
            e,
        ))?;

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

        let (repository, _outcome) = fetch.fetch_only(Discard, cancel).map_err(|e| errors::system_with_internal(
            &format!("Unable to clone remote repository '{}'", repo.clone_url),
            "Make sure that your internet connectivity is working correctly, and that your local git configuration is able to clone this repo.",
            e))?;

        self.update_config(&repository, |c| {
            c.set_raw_value(&gix::config::tree::Core::BARE, "true").map_err(|e| errors::system_with_internal(
                &format!("Unable to set the 'core.bare' configuration option for repository '{}'", repo.name()),
                "Make sure that the git repository has been correctly initialized and run `git config core.bare true` to configure it correctly.",
                e))?;

            Ok(())
        })?;

        let head_id = repository.head_id().map_err(|e| errors::user_with_internal(
            &format!("The repository '{}' did not have a valid HEAD, which may indicate that there is something wrong with the source repository.", &repo.clone_url),
            "Make sure that the remote repository is valid.",
            e))?;

        Ok(BackupState::New(Some(format!("at {}", head_id.to_hex()))))
    }

    #[instrument(skip(self, repo, target, cancel), err)]
    fn fetch(
        &self,
        repo: &GitRepo,
        target: &Path,
        cancel: &AtomicBool,
    ) -> Result<BackupState, errors::Error> {
        let repository = gix::open(target).map_err(|e| {
            errors::user_with_internal(
                &format!(
                    "Failed to open the repository '{}' at '{}'",
                    &repo.clone_url,
                    &target.display()
                ),
                "Make sure that the target directory is a valid git repository.",
                e,
            )
        })?;

        let original_head = repository.head_id().ok();

        let remote = repository.find_fetch_remote(Some(repo.clone_url.as_str().into())).map_err(|e| {
            errors::user_with_internal(
                &format!(
                    "Failed to find the remote '{}' in the repository '{}'",
                    repo.clone_url,
                    &target.display()
                ),
                "Make sure that the repository is correctly configured and that the remote exists.",
                e,
            )
        })?
            .with_fetch_tags(Tags::All)
            .with_refspecs(["+refs/heads/*:refs/remotes/origin/*"], gix::remote::Direction::Fetch)
            .map_err(|e| {
                errors::user_with_internal(
                    &format!(
                        "Failed to configure the remote '{}' in the repository '{}' to fetch all branches.",
                        &repo.clone_url,
                        &target.display()
                    ),
                    "Make sure that the repository is correctly configured and that the remote exists.",
                    e,
                )
            })?;

        let mut connection = remote.connect(gix::remote::Direction::Fetch).map_err(|e| {
            errors::user_with_internal(
                &format!(
                    "Unable to establish connection to remote git repository '{}'",
                    &repo.clone_url
                ),
                "Make sure that the repository is available and correctly configured.",
                e,
            )
        })?;

        Self::authenticate_connection(&mut connection, &repo.credentials);

        connection
            .prepare_fetch(Discard, Default::default())
            .map_err(|e| {
                errors::user_with_internal(
                    &format!(
                        "Unable to prepare fetch from remote git repository '{}'",
                        &repo.clone_url
                    ),
                    "Make sure that the repository is available and correctly configured.",
                    e,
                )
            })?
            .with_write_packed_refs_only(true)
            .receive(Discard, cancel)
            .map_err(|e| {
                errors::user_with_internal(
                    &format!(
                        "Unable to fetch from remote git repository '{}'",
                        &&repo.clone_url
                    ),
                    "Make sure that the repository is available and correctly configured.",
                    e,
                )
            })?;

        let head_id = repository.head_id().map_err(|e| errors::user_with_internal(
            &format!("The repository '{}' did not have a valid HEAD, which may indicate that there is something wrong with the source repository.", &repo.clone_url),
            "Make sure that the remote repository is valid.",
            e))?;

        if let Some(original_head) = original_head {
            if original_head == head_id {
                return Ok(BackupState::Unchanged(Some(format!(
                    "at {}",
                    head_id.to_hex()
                ))));
            }
        }

        Ok(BackupState::Updated(Some(format!("{}", head_id.to_hex()))))
    }

    fn authenticate_connection<T>(connection: &mut Connection<'_, '_, T>, creds: &Credentials) {
        match creds {
            Credentials::None => {}
            creds => {
                let creds = creds.clone();
                connection.set_credentials(move |a| match a {
                    Action::Get(ctx) => Ok(Some(gix::credentials::protocol::Outcome {
                        identity: match &creds {
                            Credentials::None => Account {
                                username: "".into(),
                                password: "".into(),
                            },
                            Credentials::Token(token) => Account {
                                username: token.clone(),
                                password: "".into(),
                            },
                            Credentials::UsernamePassword { username, password } => Account {
                                username: username.clone(),
                                password: password.clone(),
                            },
                        },
                        next: ctx.into(),
                    })),
                    _ => Ok(None),
                });
            }
        }
    }

    fn update_config<U>(&self, repo: &gix::Repository, mut update: U) -> Result<(), errors::Error>
    where
        U: FnMut(&mut gix::config::File<'_>) -> Result<(), errors::Error>,
    {
        let mut config = gix::config::File::from_path_no_includes(
            repo.path().join("config"),
            gix::config::Source::Local,
        )
        .map_err(|e| {
            errors::system_with_internal(
                &format!(
                    "Unable to load git configuration for repository '{}'",
                    repo.path().display()
                ),
                "Make sure that the git repository has been correctly initialized.",
                e,
            )
        })?;

        update(&mut config)?;

        let mut file = std::fs::File::create(repo.path().join("config")).map_err(|e| {
            errors::system_with_internal(
                &format!(
                    "Unable to write git configuration for repository '{}'",
                    repo.path().display()
                ),
                "Make sure that the git repository has been correctly initialized.",
                e,
            )
        })?;

        config.write_to(&mut file).map_err(|e| {
            errors::system_with_internal(
                &format!(
                    "Unable to write git configuration for repository '{}'",
                    repo.path().display()
                ),
                "Make sure that the git repository has been correctly initialized.",
                e,
            )
        })
    }
}

impl Display for GitEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "git")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "pure_tests"))]
    #[tokio::test]
    async fn test_backup() {
        let temp_dir = tempfile::tempdir().expect("a temporary directory");

        let agent = GitEngine;
        let cancel = AtomicBool::new(false);

        let repo = GitRepo::new(
            "SierraSoftworks/grey",
            "https://github.com/sierrasoftworks/grey.git",
        );

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
}
