use std::sync::{Arc, Mutex, Weak};

pub trait Listener<T>: Sync + Send {
    fn on_event(&self, event: &T);
}

pub struct ListenerManager<T> {
    // Weak so listeners don't prevent their owners from being dropped
    listeners: Mutex<Vec<Weak<dyn Listener<T>>>>,
}

impl<T> ListenerManager<T> {
    pub fn new() -> Self {
        Self {
            listeners: Mutex::new(Vec::new())
        }
    }

    pub fn add_listener(&self, listener: &Arc<dyn Listener<T>>) {
        self.listeners.lock().unwrap().push(Arc::downgrade(listener));
    }

    pub fn remove_listener(&self, listener: &Arc<dyn Listener<T>>) {
        self.listeners.lock().unwrap().retain(|weak| {
            // Keep if: still alive AND not the one we're removing
            weak.upgrade()
                .map(|arc| !Arc::ptr_eq(&arc, listener))
                .unwrap_or(false) // drop dead listeners too
        });
    }

    pub fn emit(&self, event: &T) -> usize {
        let mut cnt = 0;
        self.listeners.lock().unwrap().retain(|weak| {
            if let Some(listener) = weak.upgrade() {
                listener.on_event(event);
                cnt += 1;
                true
            } else {
                false // auto-clean dropped listeners
            }
        });
        cnt
    }
}