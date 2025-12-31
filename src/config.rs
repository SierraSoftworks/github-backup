use human_errors::ResultExt;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

use crate::{Args, policy::BackupPolicy};

#[derive(Deserialize)]
pub struct Config {
    #[serde(deserialize_with = "deserialize_cron")]
    pub schedule: Option<croner::Cron>,

    #[serde(default)]
    pub backups: Vec<BackupPolicy>,
}

impl TryFrom<&Args> for Config {
    type Error = human_errors::Error;

    fn try_from(value: &Args) -> Result<Self, Self::Error> {
        let content = std::fs::read_to_string(&value.config).wrap_err_as_user(
            format!("Failed to read the config file {}.", &value.config),
            &["Make sure that the configuration file exists and can be ready by the process."],
        )?;
        let config: Config = serde_yaml::from_str(&content).wrap_err_as_user(
            "Failed to parse your configuration file, as it is not recognized as valid YAML.",
            &["Make sure that your configuration file is formatted correctly."],
        )?;
        Ok(config)
    }
}

fn deserialize_cron<'de, D>(deserializer: D) -> Result<Option<croner::Cron>, D::Error>
where
    D: Deserializer<'de>,
{
    if let Some(s) = Deserialize::deserialize(deserializer)? {
        let s: String = s;
        return croner::Cron::from_str(&s)
            .map_err(serde::de::Error::custom)
            .map(Some);
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use rstest::rstest;

    #[rstest]
    #[case("0 0 * * *")]
    #[case("0 */5 * * *")]
    fn deserialize_cron(#[case] format: &str) {
        let config: Config = serde_yaml::from_str(&format!("schedule: {}", format)).unwrap();
        assert!(config.schedule.is_some());
    }

    #[test]
    #[cfg_attr(feature = "pure_tests", ignore)]
    fn deserialize_example_config() {
        let args = Args::parse_from([
            "github-backup",
            "--config",
            &format!(
                "{}",
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("examples")
                    .join("config.yaml")
                    .display()
            ),
        ]);

        let config: Config = Config::try_from(&args).expect("the example config should be valid");
        assert!(config.schedule.is_some());
        assert!(config.backups.iter().len() > 0);
    }
}
