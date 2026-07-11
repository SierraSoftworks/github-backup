use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

use crate::Filter;
use crate::entities::Credentials;
use crate::target::BackupTargets;

/// The number of times an individual target backup is retried if it fails
/// before the failure is reported, unless overridden by the `retries` policy
/// property. Backing up a failed target one more time before giving up smooths
/// over the transient network and remote-side failures which are common when
/// mirroring large numbers of repositories.
pub const DEFAULT_RETRIES: usize = 1;

#[derive(Deserialize, Default)]
pub struct BackupPolicy {
    pub kind: String,
    pub from: String,
    #[serde(default)]
    pub to: BackupTargets,
    #[serde(default)]
    pub credentials: Credentials,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

impl BackupPolicy {
    /// The number of times the backup of an individual target should be retried
    /// if it fails before the error is reported to the user.
    ///
    /// Configured through the optional `retries` policy property and defaulting
    /// to [`DEFAULT_RETRIES`]. A value of `0` disables retries entirely, causing
    /// the first failure to be reported immediately.
    pub fn retries(&self) -> Result<usize, crate::Error> {
        match self.properties.get("retries") {
            Some(value) => value.trim().parse().map_err(|_| {
                human_errors::user(
                    format!("The 'retries' property value '{value}' is not a valid number."),
                    &[
                        "Set the 'retries' property to a non-negative whole number, such as '1', or remove it to use the default.",
                    ],
                )
            }),
            None => Ok(DEFAULT_RETRIES),
        }
    }
}

impl Display for BackupPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.kind, self.from)
    }
}

impl Debug for BackupPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.kind, self.from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize() {
        let policy = r#"
          kind: backup
          from: source
          to: /tmp/backup
          credentials: !UsernamePassword { username: admin, password: pass }
          filter: repo.name == "my-repo"
          properties:
            key: value
        "#;
        let policy: BackupPolicy = serde_yaml::from_str(policy).unwrap();
        assert_eq!(policy.kind, "backup");
        assert_eq!(policy.from, "source");
        assert_eq!(
            policy.to,
            BackupTargets(vec![crate::target::BackupTarget::FileSystem(
                std::path::PathBuf::from("/tmp/backup")
            )])
        );
        assert_eq!(
            policy.credentials,
            Credentials::UsernamePassword {
                username: "admin".to_string(),
                password: "pass".to_string(),
            }
        );
        assert_eq!(policy.filter.raw(), "repo.name == \"my-repo\"");
        assert_eq!(policy.properties, {
            let mut map = HashMap::new();
            map.insert("key".to_string(), "value".to_string());
            map
        });

        assert_eq!(format!("{}", policy), "backup/source");
        assert_eq!(format!("{:?}", policy), "backup/source");
    }

    #[test]
    fn test_retries_default() {
        let policy = BackupPolicy::default();
        assert_eq!(
            policy
                .retries()
                .expect("the default retry count to be valid"),
            DEFAULT_RETRIES
        );
    }

    #[test]
    fn test_retries_configured() {
        let policy: BackupPolicy = serde_yaml::from_str(
            r#"
            kind: backup
            from: source
            properties:
              retries: "3"
            "#,
        )
        .unwrap();

        assert_eq!(policy.retries().expect("a valid retry count"), 3);
    }

    #[test]
    fn test_retries_zero_disables_retries() {
        let policy: BackupPolicy = serde_yaml::from_str(
            r#"
            kind: backup
            from: source
            properties:
              retries: "0"
            "#,
        )
        .unwrap();

        assert_eq!(policy.retries().expect("a valid retry count"), 0);
    }

    #[test]
    fn test_retries_invalid() {
        let policy: BackupPolicy = serde_yaml::from_str(
            r#"
            kind: backup
            from: source
            properties:
              retries: "not-a-number"
            "#,
        )
        .unwrap();

        policy
            .retries()
            .expect_err("an invalid retry count to be rejected");
    }
}
