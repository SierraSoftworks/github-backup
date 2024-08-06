use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;

use crate::entities::Credentials;
use crate::Filter;

#[derive(Deserialize)]
pub struct BackupPolicy {
    pub kind: String,
    pub from: String,
    #[serde(default = "default_backup_path")]
    pub to: PathBuf,
    #[serde(default)]
    pub credentials: Credentials,
    #[serde(default)]
    pub filter: Filter,
    #[serde(default)]
    pub properties: HashMap<String, String>,
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

fn default_backup_path() -> PathBuf {
    PathBuf::from("./backups")
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
        assert_eq!(policy.to, PathBuf::from("/tmp/backup"));
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
}
