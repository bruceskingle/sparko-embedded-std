use std::str::FromStr;

use chrono::{DateTime, Local};
use croner::Cron;
use log::info;

use crate::SparkoEmbeddedStd;


pub trait Task
{
    fn run(&mut self, sparko_embedded: &dyn SparkoEmbeddedStd) -> anyhow::Result<()>;
    fn name(&self) -> &str;
}

struct TaskHolder
{
    task: Box<dyn Task>,
    schedule: Cron,
    next_event: DateTime<Local>,
}

pub struct TaskManagerBuilder
{
    tasks: Vec<TaskHolder>,
}

impl TaskManagerBuilder
{
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
        }
    }

    pub fn add_task(&mut self, task: Box<dyn Task>, schedule_spec: &str) -> anyhow::Result<()> {
        let schedule = Cron::from_str(schedule_spec)?;
        // We are doing this here to validate the schedule spec early and avoid adding tasks with invalid schedules to the manager
        let next_event = schedule.find_next_occurrence(&Local::now(), false)?;
        info!("Task {} added with schedule {} which means {}", task.name(), schedule_spec, schedule.describe());
        self.tasks.push(TaskHolder{task, schedule, next_event});
        Ok(())
    }

    pub fn with_task(mut self, task: Box<dyn Task>, schedule_spec: &str) -> anyhow::Result<Self> {
        self.add_task(task, schedule_spec)?;
        Ok(self)
    }

    pub fn build(mut self) -> TaskManager {
        self.tasks.shrink_to_fit();
        TaskManager {
            tasks: self.tasks,
        }
    }
}

pub struct TaskManager
{
    tasks: Vec<TaskHolder>,
}

impl TaskManager
{
    pub fn builder() -> TaskManagerBuilder {
        TaskManagerBuilder::new()
    }

    pub fn run(&mut self, sparko_embedded: &dyn SparkoEmbeddedStd) -> anyhow::Result<()> {
        info!("Starting task manager with {} tasks", self.tasks.len());
        if self.tasks.is_empty() {
            anyhow::bail!("No tasks to run.");
        }

        let now = Local::now();
        let mut i = 0;

        // Calculate the next event for all tasks
        while i < self.tasks.len() {
            match self.tasks[i].schedule.find_next_occurrence(&now, false) {
                Ok(value) => {
                    self.tasks[i].next_event = value;
                    i += 1;
                },
                Err(error) => {
                    log::error!("Failed to calculate next occurrence for task {}: {}", self.tasks[i].task.name(), error);
                    self.tasks.swap_remove(i);
                    // Don't increment i since we swapped in a new task at index i
                },
            }
        }

        if self.tasks.is_empty() {
            anyhow::bail!("No schedulable tasks to run.");
        }


        loop {
            info!("(TaskManager::run() top of loop");

            // Find the task with the earliest next event
            let mut next_task_id = 0;
            i = 1;
            while i < self.tasks.len() {
                if self.tasks[i].next_event < self.tasks[next_task_id].next_event {
                    next_task_id = i;
                }
                i += 1;
            }
        
            let mut now = Local::now();
            while self.tasks[next_task_id].next_event > now {
                log::info!("Next task {} scheduled for {}, waiting...", self.tasks[next_task_id].task.name(), self.tasks[next_task_id].next_event);
                std::thread::sleep((self.tasks[next_task_id].next_event - now).to_std().unwrap());
                now = Local::now();
            }
            log::info!("Running task: {}", self.tasks[next_task_id].task.name());
            if let Err(error) = self.tasks[next_task_id].task.run(sparko_embedded) {
                log::error!("Error running task {}: {}", self.tasks[next_task_id].task.name(), error);
            }
            // Update next event after running
            match self.tasks[next_task_id].schedule.find_next_occurrence(&self.tasks[next_task_id].next_event, false) {
                Ok(value) => self.tasks[next_task_id].next_event = value,
                Err(error) => {
                    log::error!("Failed to calculate next occurrence for task {}: {}", self.tasks[next_task_id].task.name(), error);
                    self.tasks.remove(next_task_id);

                    if self.tasks.is_empty() {
                        // log::warn!("No tasks to run.");
                        anyhow::bail!("No schedulable tasks to run.");
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    // struct MockSparkoEmbeddedStd {

    // }

    // impl SparkoEmbeddedStd for MockSparkoEmbeddedStd {

    // }

    struct MockTask {
    }

    impl MockTask {
        fn new() -> Box<Self> {
            Box::new(Self {
                
            })
        }
    }

    // Only implement Task for tests
    impl Task for MockTask {
        fn run(&mut self, _sparko_embedded: &dyn SparkoEmbeddedStd) -> anyhow::Result<()> {
            Ok(())
        }

        // fn get_schedule(&self) -> super::Cron {
        //     self.schedule.clone()
        // }
        
        fn name(&self) -> &str {
            "MockTask"
        }
    }

    mod task_manager_tests {
        use super::*;

        #[test]
        fn test_new_task_manager() {
            let manager = TaskManager::builder().build();
            assert_eq!(manager.tasks.len(), 0);
        }

        #[test]
        fn test_add_first_task() {
            let manager = TaskManager::builder()
            .with_task(MockTask::new(), "* * * * * *").unwrap() // Add a task that runs every minute
            .build();
            
            assert_eq!(manager.tasks.len(), 1);
        }

        // #[test]
        // fn test_add_multiple_tasks_with_different_schedules() {
        //     let mut manager = TaskManager::new();
            
        //     // Add task that runs in 1 minute
        //     let task1 = MockTask::new("* * * * * *");
        //     manager.add_task(task1);
            
        //     // Add task that runs in 2 minutes (should become next_task_id)
        //     let future_time = Local::now() + Duration::minutes(2);
        //     let cron_expr = format!("{} {} {} {} {} *", 
        //         future_time.minute(),
        //         future_time.hour(),
        //         future_time.day(),
        //         future_time.month(),
        //         future_time.weekday().number_from_monday()
        //     );
        //     let task2 = MockTask::new(&cron_expr);
        //     manager.add_task(task2);
            
        //     assert_eq!(manager.tasks.len(), 2);
        //     assert!(manager.next_task_id.is_some());
        //     // The first task (index 0) should be the next one since it runs sooner
        //     assert_eq!(manager.next_task_id.unwrap(), 0);
        // }

        // #[test]
        // fn test_task_schedule() {
        //     let task = MockTask::new("* * * * * *"); // Every second
        //     let schedule = task.get_schedule();
            
        //     // Test that we can find next occurrence
        //     let now = Local::now();
        //     let next = schedule.find_next_occurrence(&now, false);
        //     assert!(next.is_ok());
            
        //     let next_time = next.unwrap();
        //     assert!(next_time > now);
        // }
    }
}   