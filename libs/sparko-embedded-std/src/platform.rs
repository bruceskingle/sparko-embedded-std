use crate::task::scheduler::ScheduledTask;

pub trait Platform {}

pub trait PlatformInitializer {
    type Platform: Platform;

    fn add_task(
        &mut self,
        task_initializer: Box<dyn ScheduledTask<Self::Platform>>,
        schedule_spec: &str,
    ) -> anyhow::Result<()>;
}
