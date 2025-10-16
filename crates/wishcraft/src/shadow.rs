use crate::schema::Wish;
use anyhow::Result;

#[cfg(feature = "sim")]
pub trait ShadowRunner {
    type Snapshot;
    type Diff;
    fn shadow_run(&self, wish: &Wish, snap: &Self::Snapshot) -> Result<Self::Diff>;
}
