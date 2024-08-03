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
                  yield Ok((entity, BackupState::Skipped));
                  continue;
              }

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