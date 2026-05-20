use std::{sync::mpsc::{self, Receiver, Sender}, time::Duration};

pub type ProcessId = usize;

pub trait ThreadTask {
    fn start(self, task_client: TaskClient);
}

pub struct TaskConfig {
    heart_beat_freq: Duration,
}

pub enum TaskType {
    Scheduled,
    Thread,
}

pub enum DeclaredProcessStatus {
    Initializing,
    OK,
    Warning,
    Error,
    Failed,
    Terminated,
}

pub enum ObservedProcessStatus {
    Starting,
    Running,
    Delayed,
    Dead
}

pub enum TaskControlMessage {
    Shutdown,
    Quiesce,
    Start
}

pub enum DeclaredTaskStatus {
    Initializing,
    Idle,
    Starting,
    Running,
    ShuttingDown,
}

pub enum TaskStatusMessage {
    ProcessStatus{pid: ProcessId, status: DeclaredProcessStatus},
    TaskStatus(DeclaredTaskStatus),
}

/// The connection to the MCP held by a Task
pub struct TaskClient {
    control_receiver: Receiver<TaskControlMessage>,
    status_sender: Sender<TaskStatusMessage>,
}

struct TaskHolder {
    task: Box<dyn ThreadTask>,
    current_processes: Vec<ProcessHolder>,
    control_sender: Sender<TaskControlMessage>,
    status_receiver: Receiver<TaskStatusMessage>,
    declared_status: DeclaredTaskStatus,
}

struct ProcessHolder {
    id: ProcessId,
    declared_status: DeclaredProcessStatus,
    observed_status: ObservedProcessStatus,
}

pub struct MasterControlProgramBuilder {
    tasks: Vec<TaskHolder>,
}

impl MasterControlProgramBuilder {
    fn new() -> MasterControlProgramBuilder {
        MasterControlProgramBuilder {
            tasks: Vec::new(),
        }
    }

    pub fn add_task(&mut self, task: Box<dyn ThreadTask>, config: TaskConfig) -> anyhow::Result<()> {
        let (control_sender, control_receiver) = mpsc::channel();
        let (status_sender, status_receiver) = mpsc::channel();

        let task_holder = TaskHolder {
            task,
            current_processes: Vec::new(),
            control_sender,
            status_receiver,
            declared_status: DeclaredTaskStatus::Initializing,
        };

        let task_client = TaskClient {
            control_receiver,
            status_sender,
        };
        Ok(())
    }

    pub fn with_task(mut self, task: Box<dyn ThreadTask>, config: TaskConfig) -> anyhow::Result<Self> {
        self.add_task(task, config)?;
        Ok(self)
    }

    pub fn build(mut self) -> MasterControlProgram {
        self.tasks.shrink_to_fit();

        MasterControlProgram {
            tasks: self.tasks,
        }
    }
}

pub struct MasterControlProgram {
    tasks: Vec<TaskHolder>,
}

impl MasterControlProgram {
    pub fn builder() -> MasterControlProgramBuilder {
        MasterControlProgramBuilder::new()
    }
}