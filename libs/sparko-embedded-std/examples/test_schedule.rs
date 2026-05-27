use log::info;
use sparko_embedded_std::platform::Platform;
use sparko_embedded_std::task::scheduler::{ScheduledTask, TaskScheduler};

struct TestPlatform {}

impl Platform for TestPlatform {}

struct TestTask {
    name: &'static str,
}

impl TestTask {
    fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl ScheduledTask<TestPlatform> for TestTask {
    fn run(&mut self, _sparko_embedded: &mut TestPlatform) -> anyhow::Result<()> {
        log::info!("TestTask: {}", self.name);
        Ok(())
    }

    fn name(&self) -> &str {
        self.name
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    info!("Test schedule...");

    let mut task_manager = TaskScheduler::builder()
        .with_task(Box::new(TestTask::new("Every Sec")), "* * * * * *")?
        .with_task(Box::new(TestTask::new("Every Min")), "0 * * * * *")?
        .build();

    task_manager.run(&mut TestPlatform {})?;

    Ok(())
}
