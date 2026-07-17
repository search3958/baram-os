#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;

pub struct ProcessManager {
    processes: Vec<Process>,
}

pub struct Process {
    pub id: u32,
    pub name: String,
    pub subsystem_idx: usize,
    pub state: ProcessState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Created,
    Running,
    Paused,
    Stopped,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }

    pub fn create_process(&mut self, name: &str, subsystem_idx: usize) -> u32 {
        let id = self.processes.len() as u32;
        self.processes.push(Process {
            id,
            name: String::from(name),
            subsystem_idx,
            state: ProcessState::Created,
        });
        id
    }

    pub fn start_process(&mut self, id: u32) -> bool {
        if let Some(proc) = self.processes.iter_mut().find(|p| p.id == id) {
            if proc.state == ProcessState::Created || proc.state == ProcessState::Paused {
                proc.state = ProcessState::Running;
                return true;
            }
        }
        false
    }

    pub fn pause_process(&mut self, id: u32) -> bool {
        if let Some(proc) = self.processes.iter_mut().find(|p| p.id == id) {
            if proc.state == ProcessState::Running {
                proc.state = ProcessState::Paused;
                return true;
            }
        }
        false
    }

    pub fn stop_process(&mut self, id: u32) -> bool {
        if let Some(proc) = self.processes.iter_mut().find(|p| p.id == id) {
            if proc.state != ProcessState::Stopped {
                proc.state = ProcessState::Stopped;
                return true;
            }
        }
        false
    }

    pub fn get_running_processes(&self) -> Vec<u32> {
        self.processes
            .iter()
            .filter(|p| p.state == ProcessState::Running)
            .map(|p| p.id)
            .collect()
    }

    pub fn get_process(&self, id: u32) -> Option<&Process> {
        self.processes.iter().find(|p| p.id == id)
    }

    pub fn get_process_by_subsystem(&self, subsystem_idx: usize) -> Option<&Process> {
        self.processes.iter().find(|p| p.subsystem_idx == subsystem_idx)
    }

    pub fn count(&self) -> usize {
        self.processes.len()
    }
}
