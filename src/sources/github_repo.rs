use std::{sync::atomic::AtomicBool, sync::Arc};

use reqwest::{header::LINK, Method, StatusCode, Url};
use tokio_stream::{Stream, StreamExt};

use crate::{
    entities::{Credentials, GitRepo},
    errors::{self, ResponseError},
    policy::BackupPolicy,
    BackupSource,
};

#[derive(Clone)]
pub struct GitHubRepoSource {
    client: Arc<reqwest::Client>,
}

impl BackupSource<GitRepo> for GitHubRepoSource {
    fn kind(&self) -> &str {
        "github/repo"
    }

    fn validate(&self, policy: &BackupPolicy) -> Result<(), crate::Error> {
        let target = policy.from.as_str().trim_matches('/');
        match target {
            t if t.is_empty() => Err(errors::user(
                "The target field is required for GitHub repository backups.",
                "Please provide a target field in the policy using the format 'users/<username>' or 'orgs/<orgname>'.",
            )),

            t if t.chars().filter(|c| *c == '/').count() > 1 => Err(errors::user(
                &format!("The target field '{target}' contains too many segments."),
                "Please provide a target field in the policy using the format 'users/<username>' or 'orgs/<orgname>'.",
            )),

            t if !t.starts_with("users/") || !t.starts_with("orgs/") => Err(errors::user(
                &format!("The target field '{target}' does not include a valid user or org specifier."),
                "Please specify either 'users/<username>' or 'orgs/<orgname>' as your target.",
            )),

            _ => Ok(()),
        }
    }

    fn load<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<GitRepo, errors::Error>> + 'a {
        let url = format!(
            "{}/{}/repos",
            policy
                .properties
                .get("api_url")
                .as_deref()
                .unwrap_or(&"https://api.github.com".to_string())
                .trim_end_matches('/'),
            &policy.from.trim_matches('/')
        );

        self.get_paginated::<GitHubRepo>(url, &policy.credentials, cancel)
            .map(|result| {
                result
                    .map(|repo| repo.into())
                    .map(|repo: GitRepo| repo.with_credentials(policy.credentials.clone()))
            })
    }
}

impl GitHubRepoSource {
    pub fn new() -> Self {
        GitHubRepoSource {
            client: Arc::new(reqwest::Client::new()),
        }
    }

    fn get_paginated<'a, T: serde::de::DeserializeOwned + 'a>(
        &'a self,
        page_url: String,
        creds: &'a Credentials,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<T, errors::Error>> + 'a {
        async_stream::try_stream! {
          let mut page_url = Some(page_url);

          while let Some(url) = page_url {
              if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  Err(errors::user(
                      "The backup operation was cancelled by the user. Only partial data may have been backed up.",
                      "Allow the backup to complete fully before cancelling again."))?;
              }

              let resp = self.call(Method::GET, &url, creds, |r| r, cancel).await?;

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

              match resp.json::<Vec<T>>().await {
                Ok(results) => {
                  for result in results.into_iter() {
                      yield result;
                  }
                },
                Err(err) => {
                  Err(errors::system_with_internal(
                      &format!("Unable to parse GitHub response into the expected structure when requesting '{}'.", &url),
                      "Please report this issue to us on GitHub.",
                      err))?;
                }
              }
          }
        }
    }

    async fn call<B>(
        &self,
        method: Method,
        url: &str,
        creds: &Credentials,
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

        req = match creds {
            Credentials::None => req,
            Credentials::Token(token) => req.basic_auth(token, Some("".to_string())),
            Credentials::UsernamePassword { username, password } => {
                req.basic_auth(username, Some(password))
            }
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
            let err = ResponseError::with_body(resp).await;
            Err(errors::user_with_internal(
                &format!(
                    "The GitHub API returned an error response with status code {}.",
                    err.status_code
                ),
                "Please check the error message below and try again.",
                err,
            ))
        }
    }
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
pub struct GitHubRepo {
    name: String,
    full_name: String,
    default_branch: String,
    clone_url: String,
    archived: bool,
    fork: bool,
    private: bool,
    size: u64,
}

impl Into<GitRepo> for GitHubRepo {
    fn into(self) -> GitRepo {
        GitRepo::new(self.full_name, self.clone_url)
            .with_optional_tag(if self.size == 0 {
                Some(crate::entities::git_repo::TAG_EMPTY)
            } else {
                None
            })
            .with_optional_tag(if self.archived {
                Some(crate::entities::git_repo::TAG_ARCHIVED)
            } else {
                None
            })
            .with_optional_tag(if self.fork {
                Some(crate::entities::git_repo::TAG_FORK)
            } else {
                None
            })
            .with_optional_tag(if self.private {
                Some(crate::entities::git_repo::TAG_PRIVATE)
            } else {
                None
            })
    }
}
