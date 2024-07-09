use std::path::PathBuf;

use serde::Deserialize;

use crate::{errors, Args};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default="default_github_api_url")]
    pub github_api_url: String,

    #[serde(default)]
    pub github_token: Option<String>,

    #[serde(default="default_backup_path")]
    pub backup_path: PathBuf,

    #[serde(default)]
    pub backups: Vec<BackupPolicyConfig>,
}

impl TryFrom<&Args> for Config {
    type Error = errors::Error;

    fn try_from(value: &Args) -> Result<Self, Self::Error> {
        let content = std::fs::read_to_string(&value.config)
            .map_err(|e| errors::user_with_internal(
                &format!("Failed to read the config file {}.", &value.config),
                "Make sure that the configuration file exists and can be ready by the process.", e))?;
        let config: Config = serde_yaml::from_str(&content)
            .map_err(|e| errors::user_with_internal(
                "Failed to parse your configuration file, as it is not recognized as valid YAML.",
                "Make sure that your configuration file is formatted correctly.",
            e))?;

        Ok(config)
    }
}

#[derive(Debug, Deserialize)]
pub struct BackupPolicyConfig {
    pub org: String,
    #[serde(default)]
    pub filters: Vec<RepoFilter>,
}

#[derive(Debug, Deserialize)]
pub enum RepoFilter {
    Include(Vec<String>),
    Exclude(Vec<String>),
    Public,
    Private,
    NonEmpty,
    Fork,
    NonFork,
    Archived,
    NonArchived,
}

fn default_github_api_url() -> String {
    "https://api.github.com".to_string()
}

fn default_backup_path() -> PathBuf {
    PathBuf::from("./backups")
}