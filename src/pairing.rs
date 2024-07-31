use std::{marker::PhantomData, sync::atomic::AtomicBool};

use tokio::task::JoinSet;
use tokio_stream::Stream;
use tracing::Instrument;

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
        T: BackupEngine<E> + 'static,
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
        async_stream::try_stream! {
          let span = tracing::info_span!("backup.policy", kind = self.source.kind(), policy = %policy).entered();

          let mut join_set: JoinSet<Result<(E, BackupState), crate::Error>> = JoinSet::new();

          for await entity in self.source.load(policy, cancel) {
              while join_set.len() >= self.concurrency_limit {
                yield join_set.join_next().await.unwrap().unwrap()?;
              }

              if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                  break;
              }

              let entity = entity?;
              if self.dry_run || policy.filters.iter().any(|f| !entity.matches(f)) {
                  yield (entity, BackupState::Skipped);
                  continue;
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
            yield fut.unwrap()?;
          }
        }
    }
}
