
use log::info;
use sparko_embedded_std::SparkoEmbeddedStd;
use sparko_embedded_std::task::{Task, TaskManager};

struct TestSparkoEmbeddedStd {

}

impl SparkoEmbeddedStd for TestSparkoEmbeddedStd {

}

struct TestTask {
    name: &'static str,
}

impl TestTask {
    fn new(name: &'static str) -> Self {
        Self {
            name,
        }
    }
}

impl Task for TestTask {
    fn run(&mut self, _sparko_embedded: &dyn SparkoEmbeddedStd) -> anyhow::Result<()> {
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

    let mut task_manager = TaskManager::builder()
        .with_task(Box::new(TestTask::new("Every Sec")), "* * * * * *")?
        .with_task(Box::new(TestTask::new("Every Min")), "0 * * * * *")?
        .build();

        task_manager.run(&TestSparkoEmbeddedStd {})?;
    
    Ok(())
}