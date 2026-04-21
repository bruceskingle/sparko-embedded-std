use std::sync::{Condvar, Mutex};
use std::time::Duration;

pub mod tz;
pub mod task;
pub mod config;
pub mod config_manager;
pub mod problem;
pub mod http_server;
pub mod feature;
pub mod command;

pub trait SparkoEmbeddedStd {
}

struct Shared<T> {
    config: T,
    updated: bool,
}

pub struct WaitNotify<T: Clone> {
    shared: std::sync::Mutex<Shared<T>>,
    notify: std::sync::Condvar,
}

impl<T: Clone> WaitNotify<T> {
    pub fn new(config: T) -> Self {
        Self {
            shared: Mutex::new(Shared { config, updated: false }),
            notify: Condvar::new(),
        }
    }

    pub fn update(&self, new_config: T) {
        let mut shared = self.shared.lock().unwrap();
        shared.config = new_config;
        shared.updated = true;
        self.notify.notify_all();
    }

    pub fn update_if(&self, new_config: T, condition: impl Fn(&T) -> bool) {
        let mut shared = self.shared.lock().unwrap();
        if condition(&shared.config) {
            shared.config = new_config;
            shared.updated = true;
            self.notify.notify_all();
        }
    }

    pub fn wait(&self) -> T {
        let mut shared = self.shared.lock().unwrap();
        while !shared.updated {
            shared = self.notify.wait(shared).unwrap();
        }
        shared.updated = false; // Reset the flag
        shared.config.clone()
    }

    pub fn wait_update(&self, timeout: Duration) -> Option<T> {
        let mut shared = self.shared.lock().unwrap();

        (shared, _) = self.notify.wait_timeout(shared, timeout).unwrap();
        
        if shared.updated {
            shared.updated = false; // Reset the flag
            Some(shared.config.clone())
        } else {
            None
        }
    }
}