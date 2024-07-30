use serde::{Deserialize, Deserializer};

use crate::{errors, policy::BackupPolicy, Args};

#[derive(Deserialize)]
pub struct Config {
    #[serde(deserialize_with = "deserialize_cron")]
    pub schedule: Option<croner::Cron>,

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
