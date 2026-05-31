use std::sync::atomic::{AtomicBool, Ordering};

use tracing_batteries::prelude::*;

use crate::{
    BackupEntity,
    engines::BackupState,
    entities::{Credentials, GitRepo},
    helpers::forgejo::{ForgejoClient, MigrateRepoOptions},
    target::RemoteTarget,
};

/// An engine which mirrors Git repositories onto a Forgejo instance using its
/// migration API.
#[derive(Clone, Default)]
pub struct ForgejoRepoEngine {
    client: ForgejoClient,
}

impl ForgejoRepoEngine {
    #[tracing::instrument(skip(self, entity, target, cancel), fields(entity = %entity))]
    pub async fn backup(
        &self,
        entity: &GitRepo,
        target: &RemoteTarget,
        cancel: &AtomicBool,
    ) -> Result<BackupState, crate::Error> {
        if cancel.load(Ordering::Relaxed) {
            return Ok(BackupState::Skipped);
        }

        let repo_name = forgejo_repo_name(entity.name());

        if self.client.repo_exists(target, &repo_name).await? {
            trace!("Repository {repo_name} already exists, requesting a mirror-sync.");
            self.client.mirror_sync(target, &repo_name).await?;
            return Ok(BackupState::Updated(Some(format!(
                "mirror-sync {}/{}",
                target.owner, repo_name
            ))));
        }

        trace!("Repository {repo_name} does not exist, migrating it as a mirror.");
        let options = MigrateRepoOptions::new(
            entity.clone_url.clone(),
            target.owner.clone(),
            repo_name.clone(),
        );

        let options = match &entity.credentials {
            Credentials::None => options,
            Credentials::Token(token) => options.with_auth_token(token.clone()),
            Credentials::UsernamePassword { username, password } => {
                options.with_basic_auth(username.clone(), password.clone())
            }
        };

        self.client.migrate_repo(target, &options).await?;
        Ok(BackupState::New(Some(format!(
            "migrated to {}/{}",
            target.owner, repo_name
        ))))
    }
}

/// Forgejo repository names cannot contain a `/`, so we use the final path
/// segment of the source repository's name as the Forgejo repository name.
fn forgejo_repo_name(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}
