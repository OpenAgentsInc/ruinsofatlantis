use anyhow::Result;

#[cfg(feature = "server")]
pub trait WishExecutor {
    type Diff;
    /// Apply a staged diff transactionally; returns a rollback anchor id.
    fn apply_staged(&mut self, diff: Self::Diff) -> Result<String>;
}
