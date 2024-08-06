use std::{marker::PhantomData, sync::atomic::AtomicBool};

use tokio::task::JoinSet;
use tokio_stream::Stream;
use tracing::Instrument as _;

use crate::{
    engines::{BackupEngine, BackupState},
    BackupEntity, BackupPolicy, BackupSource,
};

pub struct Pairing<E: BackupEntity, S: BackupSource<E>, T: BackupEngine<E>> {
    pub source: S,
    pub target: T,
    pub dry_run: bool,
    pub concurrency_limit: usize,
    _entity: PhantomData<E>,
}

impl<
        E: BackupEntity + Send + Sync + 'static,
        S: BackupSource<E> + Send + Sync + 'static,
        T: BackupEngine<E> + Send + Sync + Clone + 'static,
    > Pairing<E, S, T>
{
    pub fn new(source: S, target: T) -> Self {
        Self {
            source,
            target,
            dry_run: false,
            concurrency_limit: 10,
            _entity: Default::default(),
        }
    }

    pub fn with_dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn with_concurrency_limit(self, concurrency_limit: usize) -> Self {
        if concurrency_limit == 0 {
            self
        } else {
            Self {
                concurrency_limit,
                ..self
            }
        }
    }

    pub fn run<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'static AtomicBool,
    ) -> impl Stream<Item = Result<(E, BackupState), crate::Error>> + 'a {
        async_stream::stream! {
          let span = tracing::info_span!("backup.policy", kind = self.source.kind(), policy = %policy).entered();

          match self.source.validate(policy) {
            Ok(_) => {},
            Err(e) => {
              yield Err(e);
              return;
            }
          }

          let mut join_set: JoinSet<Result<(E, BackupState), crate::Error>> = JoinSet::new();

          for await entity in self.source.load(policy, cancel) {
              while join_set.len() >= self.concurrency_limit {
                yield join_set.join_next().await.unwrap().unwrap();
              }

              if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  break;
              }

              let entity = entity?;
              if self.dry_run {
                  eprintln!("Would backup {entity} to {}", &policy.to.display());
                  yield Ok((entity, BackupState::Skipped));
                  continue;
              }

              match policy.filter.matches(&entity) {
                Ok(true) => {},
                Ok(false) => {
                  eprintln!("Skipping backup of {entity} as it did not match the filter {}", &policy.filter);
                  yield Ok((entity, BackupState::Skipped));
                  continue;
                },
                Err(e) => {
                  yield Err(e);
                  continue;
                }
              }

              {
                let span = tracing::info_span!(parent: &span, "backup.step", item=%entity);
                let target = self.target.clone();
                let to = policy.to.clone();
                join_set.spawn(async move {
                    target.backup(&entity, to.as_path(), cancel).instrument(span).await.map(|state| (entity, state))
                });
              }
          }

          while let Some(fut) = join_set.join_next().await {
            yield fut.unwrap();
          }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rstest::rstest;

    use crate::entities::GitRepo;

    use super::*;

    static CANCEL: AtomicBool = AtomicBool::new(false);

    fn load_test_file<T: serde::de::DeserializeOwned>(
        name: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join(name);
        let json = std::fs::read_to_string(path)?;
        let value = serde_json::from_str(&json)?;
        Ok(value)
    }

    struct MockRepoSource;

    impl BackupSource<GitRepo> for MockRepoSource {
        fn kind(&self) -> &str {
            "mock"
        }

        fn validate(&self, _policy: &BackupPolicy) -> Result<(), crate::Error> {
            Ok(())
        }

        fn load<'a>(
            &'a self,
            _policy: &'a BackupPolicy,
            _cancel: &'a AtomicBool,
        ) -> impl Stream<Item = Result<GitRepo, crate::Error>> + 'a {
            async_stream::stream! {
              let repos: Vec<crate::helpers::github::GitHubRepo> = load_test_file("github.repos.0.json").unwrap();
              for repo in repos {
                yield Ok(GitRepo::new(repo.full_name.as_str(), repo.clone_url.as_str())
                    .with_metadata_source(&repo));
              }
            }
        }
    }

    #[derive(Clone)]
    struct MockEngine;

    #[async_trait::async_trait]
    impl BackupEngine<GitRepo> for MockEngine {
        async fn backup<P: AsRef<Path> + Send>(
            &self,
            entity: &GitRepo,
            _target: P,
            _cancel: &AtomicBool,
        ) -> Result<BackupState, crate::Error> {
            Ok(BackupState::New(Some(entity.name.clone())))
        }
    }

    #[rstest]
    #[case("true", 30)]
    #[case("false", 0)]
    #[case("repo.fork", 19)]
    #[case("!repo.fork", 11)]
    #[case("repo.empty", 2)]
    #[case("!repo.empty", 28)]
    #[case("!repo.fork && !repo.empty", 11)]
    #[tokio::test]
    async fn filtering(#[case] filter: &str, #[case] matches: usize) {
        use tokio_stream::StreamExt;

        let policy: BackupPolicy = serde_yaml::from_str(&format!(
            r#"
            kind: mock
            from: mock
            to: /tmp
            filter: '{}'
            "#,
            filter
        ))
        .unwrap();

        let source = MockRepoSource;
        let engine = MockEngine;
        let pairing = Pairing::new(source, engine)
            .with_concurrency_limit(5)
            .with_dry_run(false);

        let stream = pairing.run(&policy, &CANCEL);

        tokio::pin!(stream);

        let mut count = 0;
        while let Some(result) = stream.next().await {
            let (entity, state) = result.unwrap();
            match state {
                BackupState::New(name) if name == Some(entity.name.clone()) => {
                    count += 1;
                    continue;
                }
                BackupState::New(name) => {
                    panic!(
                        "Expected BackupState::New(Some({:?})) but got BackupState::New({:?})",
                        entity.name, name
                    );
                }
                _ => {}
            }
        }

        assert_eq!(count, matches);
    }
}
