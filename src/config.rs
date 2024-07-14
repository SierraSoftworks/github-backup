use std::path::PathBuf;

use serde::{Deserialize, Deserializer};

use crate::{errors, policy::BackupPolicy, Args};

#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub github: GitHubConfig,

    #[serde(deserialize_with = "deserialize_cron")]
    pub schedule: Option<croner::Cron>,

    #[serde(default = "default_backup_path")]
    pub backup_path: PathBuf,

    #[serde(default)]
    pub backups: Vec<BackupPolicy>,
}

impl TryFrom<&Args> for Config {
    type Error = errors::Error;

    fn try_from(value: &Args) -> Result<Self, Self::Error> {
        let content = std::fs::read_to_string(&value.config).map_err(|e| {
            errors::user_with_internal(
                &format!("Failed to read the config file {}.", &value.config),
                "Make sure that the configuration file exists and can be ready by the process.",
                e,
            )
        })?;
        let config: Config = serde_yaml::from_str(&content).map_err(|e| {
            errors::user_with_internal(
                "Failed to parse your configuration file, as it is not recognized as valid YAML.",
                "Make sure that your configuration file is formatted correctly.",
                e,
            )
        })?;

        Ok(config)
    }
}

#[derive(Debug, Deserialize)]
pub struct GitHubConfig {
    #[serde(default = "default_github_api_url")]
    pub api_url: String,

    #[serde(default)]
    pub access_token: Option<String>,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        GitHubConfig {
            api_url: default_github_api_url(),
            access_token: None,
        }
    }
}

fn default_github_api_url() -> String {
    "https://api.github.com".to_string()
}

fn default_backup_path() -> PathBuf {
    PathBuf::from("./backups")
}

fn deserialize_cron<'de, D>(deserializer: D) -> Result<Option<croner::Cron>, D::Error>
where
    D: Deserializer<'de>,
{
    if let Some(s) = Deserialize::deserialize(deserializer)? {
        let s: String = s;
        return croner::Cron::new(&s)
            .parse()
            .map_err(serde::de::Error::custom)
            .map(Some);
    }

    Ok(None)
}
