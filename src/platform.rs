
use crate::task::Task;

pub trait SparkoEmbeddedStd {
}

pub trait SparkoEmbeddedStdInitializer {
    type EmbeddedStd: SparkoEmbeddedStd;

    fn add_task(&mut self, task_initializer: Box<dyn Task<Self::EmbeddedStd>>, schedule_spec: &str) -> anyhow::Result<()>;
}
