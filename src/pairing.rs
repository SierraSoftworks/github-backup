use crate::telemetry::StreamExt;
use std::fmt::{Display, Formatter};
use std::{marker::PhantomData, sync::atomic::AtomicBool};
use tokio::task::JoinSet;
use tokio_stream::{Stream, StreamExt as _};
use tracing_batteries::prelude::*;

use crate::{
    BackupEntity, BackupPolicy, BackupSource,
    engines::{BackupEngine, BackupState},
};

pub struct Pairing<E: BackupEntity, S: BackupSource<E>, T: BackupEngine<E>> {
    pub source: S,
    pub target: T,
    pub dry_run: bool,
    pub concurrency_limit: usize,
    _entity: PhantomData<E>,
}

impl<
    E: BackupEntity + Clone + Send + Sync + 'static,
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

    pub async fn run(
        &self,
        policy: &BackupPolicy,
        handler: &dyn PairingHandler<E>,
        cancel: &'static AtomicBool,
    ) {
        let stream = self.run_all_backups(policy, cancel);
        tokio::pin!(stream);
        let mut stats = SummaryStatistics::new();
        while let Some(result) = stream.next().await {
            match result {
                Ok((entity, state)) => {
                    stats.record_state(&state);
                    handler.on_complete(policy, entity, state)
                }
                Err(e) => {
                    stats.record_error();
                    handler.on_error(policy, e)
                }
            }
        }

        stats.finish();
        handler.on_summary(policy, stats);
    }

    pub fn run_all_backups<'a>(
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

          for await entity in self.source.load(policy, cancel).trace(tracing::info_span!("backup.source.load")) {
              if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  break;
              }

              let entity = entity?;
              if self.dry_run {
                  for to in policy.to.iter() {
                      info!("Would backup {entity} to {to}");
                  }
                  yield Ok((entity, BackupState::Skipped));
                  continue;
              }

              // Some sources (such as releases) bundle several filterable
              // items into a single entity and apply the policy filter to each
              // item themselves. For those we skip the entity-level filtering
              // so that filtering keeps operating at the per-item granularity.
              if !self.source.filters_internally() {
                match policy.filter.matches(&entity) {
                  Ok(true) => {},
                  Ok(false) => {
                    yield Ok((entity, BackupState::Skipped));
                    continue;
                  },
                  Err(e) => {
                    yield Err(e);
                    continue;
                  }
                }
              }

              // A single source entity may be mirrored to several targets. We
              // load the entity once (above) and fan out one backup task per
              // configured target so we don't re-query the source for each
              // destination.
              for to in policy.to.iter() {
                while join_set.len() >= self.concurrency_limit {
                  debug!("Reached concurrency limit of {}, waiting for a task to complete", self.concurrency_limit);
                  yield join_set.join_next().await.unwrap().unwrap();
                }

                let span = tracing_batteries::prelude::info_span!(parent: &span, "backup.step", item=%entity, target=%to);
                let target = self.target.clone();
                let to = to.clone();
                let entity = entity.clone();
                join_set.spawn(async move {
                    debug!("Starting backup of {entity}");
                    target.backup(&entity, &to, cancel).await.map(|state| (entity, state))
                }.instrument(span));
              }
          }

          while let Some(fut) = join_set.join_next().await {
            yield fut.unwrap();
          }
        }
    }
}

pub struct SummaryStatistics {
    start_time: std::time::Instant,
    end_time: Option<std::time::Instant>,

    pub updated: usize,
    pub skipped: usize,
    pub unchanged: usize,
    pub new: usize,
    pub error: usize,
}

impl SummaryStatistics {
    fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            end_time: None,

            updated: 0,
            skipped: 0,
            unchanged: 0,
            new: 0,
            error: 0,
        }
    }

    fn record_state(&mut self, state: &BackupState) {
        match state {
            BackupState::New(_) => self.new += 1,
            BackupState::Unchanged(_) => self.unchanged += 1,
            BackupState::Updated(_) => self.updated += 1,
            BackupState::Skipped => self.skipped += 1,
        }
    }

    fn record_error(&mut self) {
        self.error += 1;
    }

    fn finish(&mut self) {
        self.end_time = Some(std::time::Instant::now());
    }

    pub fn duration(&self) -> std::time::Duration {
        self.end_time.unwrap_or(self.start_time) - self.start_time
    }
}

impl Display for SummaryStatistics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "New: {}, Unchanged: {}, Updated: {}, Skipped: {}, Error(s): {}",
            self.new, self.unchanged, self.updated, self.skipped, self.error
        )
    }
}

pub trait PairingHandler<E: BackupEntity> {
    fn on_complete(&self, policy: &BackupPolicy, entity: E, state: BackupState);
    fn on_error(&self, policy: &BackupPolicy, error: crate::Error);
    fn on_summary(&self, _policy: &BackupPolicy, _stats: SummaryStatistics) {}
}

#[cfg(test)]
mod tests {
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
                yield Ok(GitRepo::new(repo.full_name.as_str(), repo.clone_url.as_str(), None)
                    .with_metadata_source(&repo));
              }
            }
        }
    }

    #[derive(Clone)]
    struct MockEngine;

    #[async_trait::async_trait]
    impl BackupEngine<GitRepo> for MockEngine {
        async fn backup(
            &self,
            entity: &GitRepo,
            _target: &crate::BackupTarget,
            _cancel: &AtomicBool,
        ) -> Result<BackupState, crate::Error> {
            Ok(BackupState::New(Some(entity.name.clone())))
        }
    }

    enum MatchType {
        Equal,
        GreaterOrEqual,
    }

    #[rstest]
    #[case("true", MatchType::GreaterOrEqual, 10)]
    #[case("false", MatchType::Equal, 0)]
    #[case("repo.fork", MatchType::GreaterOrEqual, 10)]
    #[case("!repo.fork", MatchType::GreaterOrEqual, 5)]
    #[case("repo.empty", MatchType::GreaterOrEqual, 0)]
    #[case("!repo.empty", MatchType::GreaterOrEqual, 10)]
    #[case("!repo.fork && !repo.empty", MatchType::GreaterOrEqual, 5)]
    #[case("repo.stargazers >= 1", MatchType::GreaterOrEqual, 5)]
    #[case("repo.forks > 3", MatchType::GreaterOrEqual, 1)]
    #[case("repo.template", MatchType::GreaterOrEqual, 1)]
    #[case("!repo.template", MatchType::GreaterOrEqual, 10)]
    #[tokio::test]
    async fn filtering(
        #[case] filter: &str,
        #[case] match_type: MatchType,
        #[case] matches: usize,
    ) {
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

        let stream = pairing.run_all_backups(&policy, &CANCEL);

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

        match match_type {
            MatchType::Equal => assert_eq!(count, matches),
            MatchType::GreaterOrEqual => assert!(count >= matches),
        }
    }
}
