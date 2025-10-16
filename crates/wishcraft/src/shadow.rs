#[cfg(feature = "sim")]
pub trait ShadowRunner {
    type Snapshot;
    type Diff;
    fn shadow_run(
        &self,
        wish: &crate::schema::Wish,
        snap: &Self::Snapshot,
    ) -> anyhow::Result<Self::Diff>;
}
