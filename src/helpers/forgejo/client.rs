use human_errors::ResultExt;
use reqwest::{Method, StatusCode, Url};
use std::sync::Arc;

use crate::{
    entities::Credentials,
    errors::{HumanizableError as _, ResponseError},
    target::RemoteTarget,
};

use super::entities::{
    CreateReleaseOptions, CreateReleaseResult, EditReleaseOptions, MigrateRepoOptions, Release,
    Repository,
};

/// A thin client for the subset of the Forgejo REST API that we need in order
/// to mirror repositories and upload release artifacts.
#[derive(Clone)]
pub struct ForgejoClient {
    client: Arc<reqwest::Client>,

    #[cfg(test)]
    mock_replies: std::collections::HashMap<String, MockResponse>,
}

impl ForgejoClient {
    /// Returns true if the repository already exists on the Forgejo instance.
    pub async fn repo_exists(
        &self,
        target: &RemoteTarget,
        repo: &str,
    ) -> Result<bool, human_errors::Error> {
        let url = target.api_url(&format!("repos/{}/{}", target.owner, repo));
        let resp = self
            .call(Method::GET, &url, &target.credentials, |r| r)
            .await?;

        match resp.status() {
            s if s.is_success() => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            _ => {
                self.ensure_success(resp, "checking whether a repository exists")
                    .await?;
                unreachable!("ensure_success returns an error for non-success responses")
            }
        }
    }

    /// Requests that the Forgejo instance migrate (mirror) a repository.
    pub async fn migrate_repo(
        &self,
        target: &RemoteTarget,
        options: &MigrateRepoOptions,
    ) -> Result<Repository, human_errors::Error> {
        let url = target.api_url("repos/migrate");
        let resp = self
            .call(Method::POST, &url, &target.credentials, |r| r.json(options))
            .await?;
        let resp = self.ensure_success(resp, "migrating a repository").await?;
        self.parse_json(resp, &url).await
    }

    /// Triggers a synchronisation of an existing mirror repository.
    pub async fn mirror_sync(
        &self,
        target: &RemoteTarget,
        repo: &str,
    ) -> Result<(), human_errors::Error> {
        let url = target.api_url(&format!("repos/{}/{}/mirror-sync", target.owner, repo));
        let resp = self
            .call(Method::POST, &url, &target.credentials, |r| r)
            .await?;
        self.ensure_success(resp, "synchronising a mirrored repository")
            .await?;
        Ok(())
    }

    /// Fetches a release by tag, returning `None` if it does not exist.
    pub async fn get_release_by_tag(
        &self,
        target: &RemoteTarget,
        repo: &str,
        tag: &str,
    ) -> Result<Option<Release>, human_errors::Error> {
        let url = target.api_url(&format!(
            "repos/{}/{}/releases/tags/{}",
            target.owner, repo, tag
        ));
        let resp = self
            .call(Method::GET, &url, &target.credentials, |r| r)
            .await?;

        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let resp = self
            .ensure_success(resp, "fetching a release by tag")
            .await?;
        Ok(Some(self.parse_json(resp, &url).await?))
    }

    /// Creates a new release on the Forgejo instance.
    ///
    /// Forgejo responds with a 409 Conflict when a release already exists for
    /// the requested tag (which can happen even when [`get_release_by_tag`]
    /// reports the release as missing); this is surfaced as
    /// [`CreateReleaseResult::AlreadyExists`] rather than an error so callers
    /// can recover.
    pub async fn create_release(
        &self,
        target: &RemoteTarget,
        repo: &str,
        options: &CreateReleaseOptions,
    ) -> Result<CreateReleaseResult, human_errors::Error> {
        let url = target.api_url(&format!("repos/{}/{}/releases", target.owner, repo));
        let resp = self
            .call(Method::POST, &url, &target.credentials, |r| r.json(options))
            .await?;

        if resp.status() == StatusCode::CONFLICT {
            return Ok(CreateReleaseResult::AlreadyExists);
        }

        let resp = self.ensure_success(resp, "creating a release").await?;
        Ok(CreateReleaseResult::Created(
            self.parse_json(resp, &url).await?,
        ))
    }

    /// Updates an existing release on the Forgejo instance, for example to keep
    /// its release notes in sync with the source.
    pub async fn update_release(
        &self,
        target: &RemoteTarget,
        repo: &str,
        release_id: u64,
        options: &EditReleaseOptions,
    ) -> Result<Release, human_errors::Error> {
        let url = target.api_url(&format!(
            "repos/{}/{}/releases/{}",
            target.owner, repo, release_id
        ));
        let resp = self
            .call(Method::PATCH, &url, &target.credentials, |r| {
                r.json(options)
            })
            .await?;
        let resp = self.ensure_success(resp, "updating a release").await?;
        self.parse_json(resp, &url).await
    }

    /// Uploads an asset to an existing release.
    pub async fn upload_release_asset(
        &self,
        target: &RemoteTarget,
        repo: &str,
        release_id: u64,
        name: &str,
        data: Vec<u8>,
    ) -> Result<(), human_errors::Error> {
        let url = target.api_url(&format!(
            "repos/{}/{}/releases/{}/assets",
            target.owner, repo, release_id
        ));

        let name = name.to_string();
        let resp = self
            .call(Method::POST, &url, &target.credentials, move |r| {
                let part = reqwest::multipart::Part::bytes(data).file_name(name.clone());
                let form = reqwest::multipart::Form::new().part("attachment", part);
                r.query(&[("name", name.as_str())]).multipart(form)
            })
            .await?;

        self.ensure_success(resp, "uploading a release asset")
            .await?;
        Ok(())
    }

    async fn call<U: AsRef<str>, B>(
        &self,
        method: Method,
        url: U,
        creds: &Credentials,
        builder: B,
    ) -> Result<reqwest::Response, human_errors::Error>
    where
        B: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        let parsed_url: Url = url.as_ref().parse().wrap_user_err(
            format!(
                "Unable to parse Forgejo URL '{}' as a valid URL.",
                url.as_ref()
            ),
            &["Make sure that you have configured your Forgejo target's address correctly."],
        )?;

        #[cfg(test)]
        if let Some(response) = self.mock_replies.get(parsed_url.path()) {
            return Ok(response.into());
        } else if !self.mock_replies.is_empty() {
            panic!(
                "No mock response found for '{}'. Available mocks: {:?}",
                parsed_url.path(),
                self.mock_replies.keys()
            );
        }

        let mut req = self
            .client
            .request(method, parsed_url)
            .header("Accept", "application/json")
            .header("User-Agent", "SierraSoftworks/github-backup");

        req = match creds {
            Credentials::None => req,
            Credentials::Token(token) => req.header("Authorization", format!("token {token}")),
            Credentials::UsernamePassword { username, password } => {
                req.basic_auth(username, Some(password))
            }
        };

        let req = builder(req);

        req.send().await.map_err(|e| e.to_human_error())
    }

    async fn ensure_success(
        &self,
        resp: reqwest::Response,
        context: &str,
    ) -> Result<reqwest::Response, human_errors::Error> {
        if resp.status().is_success() {
            return Ok(resp);
        }

        if resp.status() == StatusCode::UNAUTHORIZED {
            return Err(human_errors::user(
                "The access token you have provided was rejected by the Forgejo API.",
                &["Make sure that your Forgejo token is valid and has not expired."],
            ));
        }

        let err = ResponseError::with_body(resp).await;
        let status = err.status_code;
        Err(human_errors::wrap_user(
            err,
            format!(
                "The Forgejo API returned an error response with status code {status} while {context}."
            ),
            &["Please check the error message below and try again."],
        ))
    }

    async fn parse_json<T: serde::de::DeserializeOwned, U: AsRef<str>>(
        &self,
        resp: reqwest::Response,
        url: U,
    ) -> Result<T, human_errors::Error> {
        resp.json().await.map_err(|e| {
            human_errors::wrap_system(
                e,
                format!(
                    "Unable to parse Forgejo's response for '{}' due to invalid JSON.",
                    url.as_ref()
                ),
                &["Please report this issue to us on GitHub."],
            )
        })
    }

    #[cfg(test)]
    pub fn mock<B: FnOnce(MockResponse) -> MockResponse>(mut self, path: &str, builder: B) -> Self {
        self.mock_replies
            .insert(path.to_string(), builder(MockResponse::new(StatusCode::OK)));
        self
    }
}

impl Default for ForgejoClient {
    fn default() -> Self {
        Self {
            client: Arc::new(reqwest::Client::new()),

            #[cfg(test)]
            mock_replies: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct MockResponse {
    pub status: StatusCode,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<String>,
}

#[cfg(test)]
impl MockResponse {
    pub fn new(status: StatusCode) -> Self {
        Self {
            status,
            headers: std::collections::HashMap::new(),
            body: None,
        }
    }

    pub fn with_status_code<S: Into<StatusCode>>(mut self, status: S) -> Self {
        self.status = status.into();
        self
    }

    #[allow(dead_code)]
    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_body<B: Into<String>>(mut self, body: B) -> Self {
        self.body = Some(body.into());
        self
    }
}

#[cfg(test)]
impl From<&MockResponse> for reqwest::Response {
    fn from(mock: &MockResponse) -> reqwest::Response {
        let mut builder = http::Response::builder().status(mock.status);

        for (key, value) in mock.headers.iter() {
            builder = builder.header(key, value);
        }

        if let Some(body) = mock.body.as_ref() {
            builder.body(body.clone()).unwrap().into()
        } else {
            builder.body("").unwrap().into()
        }
    }
}
