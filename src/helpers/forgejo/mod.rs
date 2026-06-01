mod client;
mod entities;

pub use client::ForgejoClient;
pub use entities::{CreateReleaseOptions, CreateReleaseResult, MigrateRepoOptions};

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::*;
    use crate::entities::Credentials;
    use crate::target::{RemoteTarget, RemoteTargetKind};

    fn target() -> RemoteTarget {
        RemoteTarget {
            kind: RemoteTargetKind::ForgejoRepo,
            address: "https://forgejo.example.com".to_string(),
            owner: "backups".to_string(),
            credentials: Credentials::Token("token".to_string()),
        }
    }

    #[tokio::test]
    async fn repo_exists_true() {
        let client = ForgejoClient::default().mock("/api/v1/repos/backups/example", |b| {
            b.with_body(r#"{"id": 1, "name": "example", "full_name": "backups/example"}"#)
        });

        assert!(client.repo_exists(&target(), "example").await.unwrap());
    }

    #[tokio::test]
    async fn repo_exists_false() {
        let client = ForgejoClient::default().mock("/api/v1/repos/backups/example", |b| {
            b.with_status_code(StatusCode::NOT_FOUND)
        });

        assert!(!client.repo_exists(&target(), "example").await.unwrap());
    }

    #[tokio::test]
    async fn migrate_repo() {
        let client = ForgejoClient::default().mock("/api/v1/repos/migrate", |b| {
            b.with_body(r#"{"id": 2, "name": "example", "full_name": "backups/example"}"#)
        });

        let options =
            MigrateRepoOptions::new("https://github.com/owner/example.git", "backups", "example")
                .with_auth_token("github-token");

        let repo = client.migrate_repo(&target(), &options).await.unwrap();
        assert_eq!(repo.full_name, "backups/example");
    }

    #[tokio::test]
    async fn mirror_sync() {
        let client =
            ForgejoClient::default().mock("/api/v1/repos/backups/example/mirror-sync", |b| b);

        client.mirror_sync(&target(), "example").await.unwrap();
    }

    #[tokio::test]
    async fn get_release_by_tag_found() {
        let client = ForgejoClient::default().mock(
            "/api/v1/repos/backups/example/releases/tags/v1.0",
            |b| {
                b.with_body(
                    r#"{"id": 5, "tag_name": "v1.0", "assets": [{"id": 1, "name": "binary.zip"}]}"#,
                )
            },
        );

        let release = client
            .get_release_by_tag(&target(), "example", "v1.0")
            .await
            .unwrap()
            .expect("release should exist");

        assert_eq!(release.id, 5);
        assert!(release.has_asset("binary.zip"));
        assert!(!release.has_asset("other.zip"));
    }

    #[tokio::test]
    async fn get_release_by_tag_missing() {
        let client = ForgejoClient::default()
            .mock("/api/v1/repos/backups/example/releases/tags/v1.0", |b| {
                b.with_status_code(StatusCode::NOT_FOUND)
            });

        let release = client
            .get_release_by_tag(&target(), "example", "v1.0")
            .await
            .unwrap();

        assert!(release.is_none());
    }

    #[tokio::test]
    async fn create_release() {
        let client = ForgejoClient::default().mock("/api/v1/repos/backups/example/releases", |b| {
            b.with_body(r#"{"id": 9, "tag_name": "v2.0", "assets": []}"#)
        });

        let options = CreateReleaseOptions::new("v2.0").with_prerelease(true);
        let result = client
            .create_release(&target(), "example", &options)
            .await
            .unwrap();

        match result {
            CreateReleaseResult::Created(release) => {
                assert_eq!(release.id, 9);
                assert_eq!(release.tag_name, "v2.0");
            }
            CreateReleaseResult::AlreadyExists => panic!("expected the release to be created"),
        }
    }

    #[tokio::test]
    async fn create_release_conflict() {
        let client = ForgejoClient::default().mock("/api/v1/repos/backups/example/releases", |b| {
            b.with_status_code(StatusCode::CONFLICT)
                .with_body(r#"{"message":"Release has no Tag"}"#)
        });

        let options = CreateReleaseOptions::new("v2.0");
        let result = client
            .create_release(&target(), "example", &options)
            .await
            .unwrap();

        assert!(matches!(result, CreateReleaseResult::AlreadyExists));
    }

    #[tokio::test]
    async fn upload_release_asset() {
        let client =
            ForgejoClient::default().mock("/api/v1/repos/backups/example/releases/9/assets", |b| {
                b.with_status_code(StatusCode::CREATED)
                    .with_body(r#"{"id": 1, "name": "binary.zip"}"#)
            });

        client
            .upload_release_asset(&target(), "example", 9, "binary.zip", vec![1, 2, 3])
            .await
            .unwrap();
    }
}
