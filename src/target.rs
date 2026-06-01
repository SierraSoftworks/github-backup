use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use serde::Deserialize;

use crate::entities::Credentials;

/// Describes where a backup should be written to.
///
/// This may either be a path on the local filesystem (the default, and the
/// historical behaviour) or a rich description of a remote service (such as a
/// Forgejo instance) which should receive the backup.
#[derive(Clone, Debug, PartialEq)]
pub enum BackupTarget {
    FileSystem(PathBuf),
    Remote(RemoteTarget),
}

impl Default for BackupTarget {
    fn default() -> Self {
        BackupTarget::FileSystem(PathBuf::from("./backups"))
    }
}

impl Display for BackupTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupTarget::FileSystem(path) => write!(f, "{}", path.display()),
            BackupTarget::Remote(target) => {
                write!(f, "{} ({})", target.kind, target.address)
            }
        }
    }
}

impl<'de> Deserialize<'de> for BackupTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // We support two representations for the `to` field:
        //  - a bare string/path (the historical filesystem behaviour); and
        //  - a map describing a rich backup target (such as a Forgejo instance).
        //
        // We deserialize via a visitor (rather than an untagged enum) so that the
        // underlying deserializer streams the map directly into `RemoteTarget`.
        // This is important because the nested `Credentials` enum relies on YAML
        // tags (e.g. `!Token`), which serde's untagged buffering does not support.
        struct TargetVisitor;

        impl<'de> serde::de::Visitor<'de> for TargetVisitor {
            type Value = BackupTarget;

            fn expecting(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str("a filesystem path or a backup target description")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BackupTarget::FileSystem(PathBuf::from(value)))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BackupTarget::FileSystem(PathBuf::from(value)))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let target =
                    RemoteTarget::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(BackupTarget::Remote(target))
            }
        }

        deserializer.deserialize_any(TargetVisitor)
    }
}

/// One or more [`BackupTarget`]s that a single backup policy should write to.
///
/// Accepting a list of targets allows a policy's source data (for example the
/// GitHub API) to be queried once while the resulting entities are mirrored to
/// several destinations.
#[derive(Clone, Debug, PartialEq)]
pub struct BackupTargets(pub Vec<BackupTarget>);

impl BackupTargets {
    pub fn iter(&self) -> std::slice::Iter<'_, BackupTarget> {
        self.0.iter()
    }
}

impl Default for BackupTargets {
    fn default() -> Self {
        BackupTargets(vec![BackupTarget::default()])
    }
}

impl Display for BackupTargets {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, target) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{target}")?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for BackupTargets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // The `to` field accepts a single target (a filesystem path string or a
        // remote target map) or a sequence mixing both forms. We dispatch via a
        // visitor so the underlying deserializer streams each target directly,
        // preserving support for YAML-tagged credentials.
        struct TargetsVisitor;

        impl<'de> serde::de::Visitor<'de> for TargetsVisitor {
            type Value = BackupTargets;

            fn expecting(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str("a backup target or a list of backup targets")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BackupTargets(vec![BackupTarget::FileSystem(
                    PathBuf::from(value),
                )]))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BackupTargets(vec![BackupTarget::FileSystem(
                    PathBuf::from(value),
                )]))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let target =
                    RemoteTarget::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(BackupTargets(vec![BackupTarget::Remote(target)]))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut targets = Vec::new();
                while let Some(target) = seq.next_element::<BackupTarget>()? {
                    targets.push(target);
                }
                Ok(BackupTargets(targets))
            }
        }

        deserializer.deserialize_any(TargetsVisitor)
    }
}

/// A backup target which uploads repositories and/or release artifacts to a
/// remote service (such as a Forgejo instance) using its REST API.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RemoteTarget {
    pub kind: RemoteTargetKind,
    pub address: String,
    pub owner: String,
    #[serde(default)]
    pub credentials: Credentials,
}

impl RemoteTarget {
    /// Construct the URL for a remote API endpoint relative to the service's
    /// `api/v1` base path.
    pub fn api_url(&self, path: &str) -> String {
        format!(
            "{}/api/v1/{}",
            self.address.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

/// The kind of remote target, identifying both the remote service and the kind
/// of artifact which should be uploaded to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum RemoteTargetKind {
    #[serde(rename = "forgejo/repo")]
    ForgejoRepo,
    #[serde(rename = "forgejo/release")]
    ForgejoRelease,
}

impl RemoteTargetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RemoteTargetKind::ForgejoRepo => "forgejo/repo",
            RemoteTargetKind::ForgejoRelease => "forgejo/release",
        }
    }
}

impl Display for RemoteTargetKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_filesystem_string() {
        let target: BackupTarget = serde_yaml::from_str("/tmp/backup").unwrap();
        assert_eq!(
            target,
            BackupTarget::FileSystem(PathBuf::from("/tmp/backup"))
        );
    }

    #[test]
    fn deserialize_forgejo_repo() {
        let target: BackupTarget = serde_yaml::from_str(
            r#"
            kind: forgejo/repo
            address: https://forgejo.example.com
            owner: backups
            credentials: !Token abc123
            "#,
        )
        .unwrap();

        assert_eq!(
            target,
            BackupTarget::Remote(RemoteTarget {
                kind: RemoteTargetKind::ForgejoRepo,
                address: "https://forgejo.example.com".to_string(),
                owner: "backups".to_string(),
                credentials: Credentials::Token("abc123".to_string()),
            })
        );
    }

    #[test]
    fn deserialize_forgejo_release() {
        let target: BackupTarget = serde_yaml::from_str(
            r#"
            kind: forgejo/release
            address: https://forgejo.example.com/
            owner: backups
            "#,
        )
        .unwrap();

        match target {
            BackupTarget::Remote(t) => {
                assert_eq!(t.kind, RemoteTargetKind::ForgejoRelease);
                assert_eq!(t.credentials, Credentials::None);
                assert_eq!(
                    t.api_url("repos/backups/example/mirror-sync"),
                    "https://forgejo.example.com/api/v1/repos/backups/example/mirror-sync"
                );
            }
            other => panic!("expected a remote target, got {other:?}"),
        }
    }

    #[test]
    fn default_is_filesystem() {
        assert_eq!(
            BackupTarget::default(),
            BackupTarget::FileSystem(PathBuf::from("./backups"))
        );
    }

    #[test]
    fn deserialize_targets_single_string() {
        let targets: BackupTargets = serde_yaml::from_str("/tmp/backup").unwrap();
        assert_eq!(
            targets,
            BackupTargets(vec![BackupTarget::FileSystem(PathBuf::from("/tmp/backup"))])
        );
    }

    #[test]
    fn deserialize_targets_single_remote() {
        let targets: BackupTargets = serde_yaml::from_str(
            r#"
            kind: forgejo/repo
            address: https://forgejo.example.com
            owner: backups
            "#,
        )
        .unwrap();

        assert_eq!(targets.0.len(), 1);
        assert!(matches!(targets.0[0], BackupTarget::Remote(_)));
    }

    #[test]
    fn deserialize_targets_mixed_list() {
        let targets: BackupTargets = serde_yaml::from_str(
            r#"
            - /tmp/backup
            - kind: forgejo/repo
              address: https://forgejo.example.com
              owner: backups
              credentials: !Token abc123
            "#,
        )
        .unwrap();

        assert_eq!(
            targets,
            BackupTargets(vec![
                BackupTarget::FileSystem(PathBuf::from("/tmp/backup")),
                BackupTarget::Remote(RemoteTarget {
                    kind: RemoteTargetKind::ForgejoRepo,
                    address: "https://forgejo.example.com".to_string(),
                    owner: "backups".to_string(),
                    credentials: Credentials::Token("abc123".to_string()),
                }),
            ])
        );
    }

    #[test]
    fn default_targets_is_single_filesystem() {
        assert_eq!(
            BackupTargets::default(),
            BackupTargets(vec![BackupTarget::FileSystem(PathBuf::from("./backups"))])
        );
    }
}
