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
    fn validate(&self, policy: &BackupPolicy) -> Result<(), human_errors::Error>;
    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<T, human_errors::Error>> + 'a;

    /// Indicates whether this source applies the policy's filter itself (for
    /// example, per child artifact) rather than relying on the entity-level
    /// filtering performed by the [`crate::pairing::Pairing`].
    ///
    /// Sources which bundle multiple filterable items into a single entity
    /// (such as a release and its assets) should return `true` and apply the
    /// filter to each item during [`BackupSource::load`], so that filtering
    /// continues to operate at the granularity of the individual items.
    fn filters_internally(&self) -> bool {
        false
    }
}
