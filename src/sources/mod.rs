mod github_gist;
mod github_releases;
mod github_repo;

pub use github_gist::GitHubGistSource;
pub use github_releases::GitHubReleasesSource;
pub use github_repo::GitHubRepoSource;
use tokio_stream::Stream;

use crate::{BackupEntity, BackupPolicy};
use std::sync::atomic::AtomicBool;

pub trait BackupSource<T: BackupEntity> {
    fn kind(&self) -> &str;
    fn validate(&self, policy: &BackupPolicy) -> Result<(), crate::Error>;
    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<T, crate::Error>> + 'a;
}
