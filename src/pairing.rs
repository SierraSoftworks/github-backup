use std::{marker::PhantomData, sync::atomic::AtomicBool};
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
    _entity: PhantomData<E>,
}

impl<E: BackupEntity, S: BackupSource<E>, T: BackupEngine<E>> Pairing<E, S, T> {
    pub fn new(source: S, target: T) -> Self {
        Self {
            source,
            target,
            dry_run: false,
            _entity: Default::default(),
        }
    }

    pub fn with_dry_run(self, dry_run: bool) -> Self {
        Self { dry_run, ..self }
    }

    pub fn run<'a>(
        &'a self,
        policy: &'a BackupPolicy,
        cancel: &'a AtomicBool,
    ) -> impl Stream<Item = Result<(E, BackupState), crate::Error>> + 'a {
        async_stream::try_stream! {
          let span = tracing::info_span!("backup.policy", kind = self.source.kind(), policy = %policy).entered();

          for await entity in self.source.load(policy, cancel) {
            let entity = entity?;
            if self.dry_run {
              yield (entity, BackupState::Skipped);
            } else {
              let engine = self.target.clone();
              yield engine.backup(&entity, policy.to.as_path(), cancel).instrument(
                tracing::info_span!(parent: &span, "backup.item", item=%entity)
              ).await.map(|state| (entity, state))?;
            }
          }
        }
    }
}
