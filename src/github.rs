use std::sync::Arc;

use reqwest::{header::LINK, Method, StatusCode, Url};

use crate::{config::{Config, RepoFilter}, errors};

#[derive(Clone)]
pub struct GitHubClient {
    api_url: Arc<String>,
    token: Arc<Option<String>>,

    client: Arc<reqwest::Client>,
}

impl GitHubClient {
    pub fn new<A: ToString, T: ToString>(api_url: A, token: Option<T>) -> Self {
        GitHubClient {
            api_url: Arc::new(api_url.to_string()),
            token: Arc::new(token.map(|t| t.to_string())),

            client: Arc::new(reqwest::Client::new()),
        }
    }

    pub async fn get_repos<O: AsRef<str>>(&self, org: O) -> Result<Vec<GitHubRepo>, errors::Error> {
        self.get_paginated(&format!("/orgs/{}/repos", org.as_ref())).await
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, errors::Error> {
        let url = format!("{}/{}", &self.api_url, path.trim_start_matches("/"));
        self.call(Method::GET, &url, |r| r).await?
            .json().await
            .map_err(|e|
                errors::system_with_internal(
                    &format!("Unable to parse GitHub response into the expected structure when requesting '{}/{}'.", &self.api_url, path.trim_start_matches("/")),
                    "Please report this issue to us on GitHub.",
                    e))
    }

    async fn get_paginated<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<Vec<T>, errors::Error> {
        let mut page_url = Some(format!("{}/{}", &self.api_url, path.trim_start_matches("/")));
        let mut results = Vec::new();

        while let Some(url) = page_url {
            let resp = self.call(Method::GET, &url, |r| r).await?;
            
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
    
    async fn call<B>(&self, method: Method, url: &str, builder: B) -> Result<reqwest::Response, errors::Error>
        where B: FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder
    {
        let parsed_url: Url = url.parse().map_err(|e| errors::user_with_internal(
                &format!("Unable to parse GitHub URL '{}' as a valid URL.", &url),
                "Make sure that you have configured your GitHub API correctly.",
                e))?;

        let mut req = self.client.request(Method::GET, parsed_url)
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
                "Make sure that your GitHub token is valid and has not expired."))
        } else {
            Err(resp.into())
        }
    }
}

impl From<&Config> for GitHubClient {
    fn from(config: &Config) -> Self {
        GitHubClient::new(&config.github_api_url, config.github_token.as_ref())
    }
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct GitHubRepo {
    pub name: String,
    pub full_name: String,
    pub default_branch: String,
    pub clone_url: String,
    pub archived: bool,
    pub fork: bool,
    pub private: bool,
}

impl GitHubRepo {
    pub fn matches(&self, filter: &RepoFilter) -> bool {
        match filter {
            RepoFilter::Include(names) => names.iter().any(|n| self.name.eq_ignore_ascii_case(&n)),
            RepoFilter::Exclude(names) => !names.iter().any(|n| self.name.eq_ignore_ascii_case(&n)),
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