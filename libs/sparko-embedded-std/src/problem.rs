use std::sync::{Arc, Mutex, MutexGuard};

use log::info;

pub type ProblemId = Option<usize>;

struct ProblemData {
    manager: Arc<ProblemManager>,
    id: ProblemId,
    is_clear: bool,
}

pub struct Problem {
    data: Mutex<ProblemData>,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = self.data.lock().unwrap();
        if ! data.is_clear {
            let manager_data = data.manager.data.lock().unwrap();
            if let Some(id) = data.id {
                if let Some(message) = &manager_data.problems[id] {
                    write!(f, "{}", message)
                } else {
                    write!(f, "No message")
                }
            }
            else{
                write!(f, "No message ID")
            }
        }
        else {
            Ok(())
        }
    }
}

impl Problem {
    pub fn new(manager: &Arc<ProblemManager>) -> Arc<Self> {
        Arc::new(Self {
            data: Mutex::new(ProblemData {
                manager: manager.clone(),
                id: None,
                is_clear: true,
            }),
        })
    }

    pub fn set(&self, message: &str) {
        let mut data = self.data.lock().unwrap();
        data.id = data.manager.set(data.id, message.to_string());
        data.is_clear = false;
    }

    pub fn clear(&self) {
        let mut data = self.data.lock().unwrap();
        data.manager.clear(data.id);
        data.is_clear = true;
    }

     pub fn is_clear(&self) -> bool {
        let data = self.data.lock().unwrap();
        data.is_clear
    }

     pub fn is_set(&self) -> bool {
        let data = self.data.lock().unwrap();
        ! data.is_clear
    }
}

struct ProblemManagerData {
    problems: Vec<Option<String>>,
    active_cnt: usize,
}
pub struct ProblemManager {
    data: Mutex<ProblemManagerData>
}

impl ProblemManager {
    pub fn new() -> Arc<Self> {
        Arc::new(
            ProblemManager {
                data: Mutex::new(ProblemManagerData {
                    problems: Vec::new(),
                    active_cnt: 0,
                    }
                )
            }
        )
    }

    pub fn is_empty(&self) -> bool {
        let data = self.data.lock().unwrap();

        info!("Problem is_empty, active_cnt={}", data.active_cnt);

        data.active_cnt == 0
    }

    pub fn set(&self, id: ProblemId, message: String) -> ProblemId {
        let mut data = self.data.lock().unwrap();
        if let Some(idx) = id {
            if data.problems[idx].is_none() {
                data.active_cnt += 1;
            }
            info!("Problem {} set to {}, active_cnt={}", idx, &message, data.active_cnt);
            data.problems[idx] = Some(message);

            id
        }
        else {
            let new_id = data.problems.len();

            data.active_cnt += 1;
            info!("Problem {} set to {}, active_cnt={}", new_id, &message, data.active_cnt);
            data.problems.push(Some(message));


            Some(new_id)
        }   
    }

    pub fn clear(&self, id: ProblemId) {
        if let Some(idx) = id {
            let mut data = self.data.lock().unwrap();
            if data.problems[idx].is_some() {
                data.problems[idx] = None;
                data.active_cnt -= 1;
            }
            info!("Problem {} cleared, active_cnt={}", idx, data.active_cnt);
        }
    }

    pub fn iter(&self) -> ProblemIter<'_> {
        let guard = self.data.lock().unwrap();
        ProblemIter { guard, idx: 0 }
    }
}

pub struct ProblemIter<'a> {
    guard: MutexGuard<'a, ProblemManagerData>,
    idx: usize,
}

impl<'a> Iterator for ProblemIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        info!("Problem Iterator.next(), active_cnt={} idx={}", self.guard.active_cnt, self.idx);
        while self.idx < self.guard.problems.len() {
            let item: &'a Option<String> = unsafe {
                &*self.guard.problems.as_ptr().add(self.idx)
            };
            self.idx += 1;
            if let Some(value) = item.as_deref() {
                return Some(value);
            }
        }
        None
    }
}

impl<'a> IntoIterator for &'a ProblemManager {
    type Item = &'a str;
    type IntoIter = ProblemIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_problem_manager_iterates_active_problems() {
        let manager = ProblemManager::new();
        manager.set(None, "first problem".to_string());
        let second_id = manager.set(None, "second problem".to_string());
        manager.set(None, "third problem".to_string());
        manager.clear(second_id);

        let problems: Vec<_> = manager.iter().collect();
        assert_eq!(problems, vec!["first problem", "third problem"]);
    }


    #[test]
    fn test_problem_manager_is_empty() {
        let manager = ProblemManager::new();

        assert_eq!(manager.is_empty(), true);

        let second_id = manager.set(None, "second problem".to_string());

        assert_eq!(manager.is_empty(), false);

        manager.clear(second_id);

        assert_eq!(manager.is_empty(), true);
    }

    #[test]
    fn test_problem_manager_borrowed_into_iterator() {
        let manager = ProblemManager::new();
        manager.set(None, "first problem".to_string());
        manager.set(None, "second problem".to_string());

        let problems: Vec<_> = (&*manager).into_iter().collect();
        assert_eq!(problems, vec!["first problem", "second problem"]);
    }
}
