use serde::{Deserialize, Serialize};

/// Options used when requesting that a Forgejo instance migrate (mirror) a
/// repository from another Git host.
///
/// See the Forgejo `POST /repos/migrate` endpoint for the full set of options.
#[derive(Debug, Clone, Serialize)]
pub struct MigrateRepoOptions {
    pub clone_addr: String,
    pub repo_name: String,
    pub repo_owner: String,
    pub service: String,
    pub mirror: bool,
    pub private: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl MigrateRepoOptions {
    pub fn new(
        clone_addr: impl Into<String>,
        repo_owner: impl Into<String>,
        repo_name: impl Into<String>,
    ) -> Self {
        Self {
            clone_addr: clone_addr.into(),
            repo_name: repo_name.into(),
            repo_owner: repo_owner.into(),
            service: "git".to_string(),
            mirror: true,
            private: true,
            auth_token: None,
            auth_username: None,
            auth_password: None,
            description: None,
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn with_basic_auth(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.auth_username = Some(username.into());
        self.auth_password = Some(password.into());
        self
    }
}

/// A subset of the fields returned by Forgejo when describing a repository.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
}

/// Options used when creating a release on a Forgejo instance.
#[derive(Debug, Clone, Serialize)]
pub struct CreateReleaseOptions {
    pub tag_name: String,
    pub name: String,
    pub draft: bool,
    pub prerelease: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_commitish: Option<String>,
}

impl CreateReleaseOptions {
    pub fn new(tag_name: impl Into<String>) -> Self {
        let tag_name = tag_name.into();
        Self {
            name: tag_name.clone(),
            tag_name,
            draft: false,
            prerelease: false,
            body: None,
            target_commitish: None,
        }
    }

    pub fn with_draft(mut self, draft: bool) -> Self {
        self.draft = draft;
        self
    }

    pub fn with_prerelease(mut self, prerelease: bool) -> Self {
        self.prerelease = prerelease;
        self
    }
}

/// A subset of the fields returned by Forgejo when describing a release.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub id: u64,
    #[allow(dead_code)]
    pub tag_name: String,
    #[serde(default)]
    pub assets: Vec<Attachment>,
}

impl Release {
    /// Returns true if the release already has an attachment with the given name.
    pub fn has_asset(&self, name: &str) -> bool {
        self.assets.iter().any(|a| a.name == name)
    }
}

/// A release attachment (asset) on a Forgejo instance.
#[derive(Debug, Clone, Deserialize)]
pub struct Attachment {
    #[allow(dead_code)]
    pub id: u64,
    pub name: String,
}
