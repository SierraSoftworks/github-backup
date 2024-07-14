use std::{sync::Arc, path::PathBuf, sync::atomic::AtomicBool};

use reqwest::{header::LINK, Method, StatusCode, Url};
use tracing::instrument;

use crate::{
    config::Config, errors, policy::{BackupPolicy, RepoFilter}, BackupEntity, RepositorySource
};

#[derive(Clone)]
pub struct GitHubSource {
    api_url: Arc<String>,
    token: Arc<Option<String>>,

    client: Arc<reqwest::Client>,
}

#[async_trait::async_trait]
impl RepositorySource<GitHubRepo> for GitHubSource {
    #[instrument(skip(self, cancel))]
    async fn get_repos(
        &self,
        policy: &BackupPolicy,
        cancel: &AtomicBool
    ) -> Result<Vec<GitHubRepo>, errors::Error> {
        let url = match (policy.user.as_ref(), policy.org.as_ref()) {
            (Some(user), None) => format!("/users/{}/repos", user),
            (None, Some(org)) => format!("/orgs/{}/repos", org),
            _ => return Err(errors::user(
                "You must specify either a user or an organization to backup repositories for.",
                "Please check your configuration and try again."
            ))
        };
        self
            .get_paginated(&url, cancel)
            .await
    }
}

impl GitHubSource {
    pub fn new<A: ToString, T: ToString>(api_url: A, token: Option<T>) -> Self {
        GitHubSource {
            api_url: Arc::new(api_url.to_string()),
            token: Arc::new(token.map(|t| t.to_string())),

            client: Arc::new(reqwest::Client::new()),
        }
    }

    async fn get_paginated<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        cancel: &AtomicBool,
    ) -> Result<Vec<T>, errors::Error> {
        let mut page_url = Some(format!(
            "{}/{}",
            &self.api_url,
            path.trim_start_matches('/')
        ));
        let mut results = Vec::new();

        while let Some(url) = page_url {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(errors::user(
                    "The backup operation was cancelled by the user. Only partial data may have been backed up.", 
                    "Allow the backup to complete fully before cancelling again."));
            }

            let resp = self.call(Method::GET, &url, |r| r, cancel).await?;

            if let Some(link_header) = resp.headers().get(LINK) {
                let link_header = link_header.to_str().map_err(|e| errors::system_with_internal(
                    "Unable to parse GitHub's Link header due to invalid characters, which will result in pagination failing to work correctly.",
                    "Please report this issue to us on GitHub.",
                    e))?;

                let links = parse_link_header::parse_with_rel(link_header).map_err(|e| errors::system_with_internal(
                    "Unable to parse GitHub's Link header, which will result in pagination failing to work correctly.",
                    "Please report this issue to us on GitHub.",
                    e))?;

                if let Some(next_link) = links.get("next") {
                    page_url = Some(next_link.raw_uri.clone());
                } else {
                    page_url = None;
                }
            } else {
                page_url = None;
            }

            let page_results: Vec<T> = resp.json().await
                .map_err(|e|
                    errors::system_with_internal(
                        &format!("Unable to parse GitHub response into the expected structure when requesting '{}'.", &url),
                        "Please report this issue to us on GitHub.",
                        e))?;

            results.extend(page_results);
        }

        Ok(results)
    }

    async fn call<B>(
        &self,
        method: Method,
        url: &str,
        builder: B,
        _cancel: &AtomicBool,
    ) -> Result<reqwest::Response, errors::Error>
    where
        B: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    {
        let parsed_url: Url = url.parse().map_err(|e| {
            errors::user_with_internal(
                &format!("Unable to parse GitHub URL '{}' as a valid URL.", &url),
                "Make sure that you have configured your GitHub API correctly.",
                e,
            )
        })?;

        let mut req = self
            .client
            .request(method, parsed_url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "SierraSoftworks/github-backup");

        req = if let Some(token) = self.token.as_ref() {
            req.bearer_auth(token)
        } else {
            req
        };

        let req = builder(req);

        let resp = req.send().await?;

        if resp.status().is_success() {
            Ok(resp)
        } else if resp.status() == StatusCode::UNAUTHORIZED {
            Err(errors::user(
                "The access token you have provided was rejected by the GitHub API.",
                "Make sure that your GitHub token is valid and has not expired.",
            ))
        } else {
            Err(resp.into())
        }
    }
}

impl From<&Config> for GitHubSource {
    fn from(config: &Config) -> Self {
        GitHubSource::new(&config.github.api_url, config.github.access_token.as_ref())
    }
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct GitHubRepo {
    name: String,
    full_name: String,
    default_branch: String,
    clone_url: String,
    archived: bool,
    fork: bool,
    private: bool,
}

impl BackupEntity for GitHubRepo {
    fn backup_path(&self) -> PathBuf {
        PathBuf::from(&self.full_name)
    }

    fn full_name(&self) -> &str {
        &self.full_name
    }

    fn clone_url(&self) -> &str {
        &self.clone_url
    }

    fn matches(&self, filter: &crate::policy::RepoFilter) -> bool {
        match filter {
            RepoFilter::Include(names) => names.iter().any(|n| self.name.eq_ignore_ascii_case(n)),
            RepoFilter::Exclude(names) => !names.iter().any(|n| self.name.eq_ignore_ascii_case(n)),
            RepoFilter::Public => !self.private,
            RepoFilter::Private => self.private,
            RepoFilter::NonEmpty => true,
            RepoFilter::Fork => self.fork,
            RepoFilter::NonFork => !self.fork,
            RepoFilter::Archived => self.archived,
            RepoFilter::NonArchived => !self.archived,
        }
    }
}