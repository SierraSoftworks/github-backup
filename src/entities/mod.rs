mod credentials;
pub mod git_repo;

use crate::BackupFilter;

pub use credentials::Credentials;
pub use git_repo::GitRepo;

pub trait BackupEntity: std::fmt::Display + Clone + Send {
    fn name(&self) -> &str;
    fn target_path(&self) -> std::path::PathBuf {
        self.name().into()
    }
    fn has_tag(&self, _tag: &str) -> bool {
        false
    }
    fn matches(&self, filter: &BackupFilter) -> bool {
        match filter {
            BackupFilter::Include(names) => {
                names.iter().any(|n| self.name().eq_ignore_ascii_case(n))
            }
            BackupFilter::Exclude(names) => {
                !names.iter().any(|n| self.name().eq_ignore_ascii_case(n))
            }
            BackupFilter::Is(tag) => self.has_tag(tag.as_str()),
            BackupFilter::IsNot(tag) => !self.has_tag(tag.as_str()),
        }
    }
}
