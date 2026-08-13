#![no_std]

extern crate alloc;

use crate::process::{Process, ProcessPriority, ProcessState, MAX_PROCESSES};
use alloc::vec::Vec;

pub struct Scheduler {
    processes: Vec<Option<Process>>,
    current_pid: Option<u32>,
    next_pid: u32,
    pub tick_count: u64,
    pub context_switches: u64,
}

impl Scheduler {
    pub fn new() -> Self {
        let mut processes = Vec::with_capacity(MAX_PROCESSES);
        for _ in 0..MAX_PROCESSES {
            processes.push(None);
        }

        Self {
            processes,
            current_pid: None,
            next_pid: 1,
            tick_count: 0,
            context_switches: 0,
        }
    }

    pub fn create_process(&mut self, name: &str, priority: ProcessPriority) -> Option<u32> {
        let pid = self.next_pid;
        self.next_pid += 1;

        if pid as usize >= MAX_PROCESSES {
            return None;
        }

        let mut proc = Process::new(pid, name, priority);
        proc.state = ProcessState::Ready;
        self.processes[pid as usize] = Some(proc);

        Some(pid)
    }

    pub fn get_process(&self, pid: u32) -> Option<&Process> {
        if (pid as usize) < MAX_PROCESSES {
            self.processes[pid as usize].as_ref()
        } else {
            None
        }
    }

    pub fn get_process_mut(&mut self, pid: u32) -> Option<&mut Process> {
        if (pid as usize) < MAX_PROCESSES {
            self.processes[pid as usize].as_mut()
        } else {
            None
        }
    }

    pub fn terminate_process(&mut self, pid: u32, exit_code: i32) {
        if let Some(proc) = &mut self.processes[pid as usize] {
            proc.state = ProcessState::Terminated;
            proc.exit_code = exit_code;
        }

        if self.current_pid == Some(pid) {
            self.current_pid = None;
        }
    }

    pub fn block_process(&mut self, pid: u32, wait_channel: u32) {
        if let Some(proc) = &mut self.processes[pid as usize] {
            if proc.state == ProcessState::Running || proc.state == ProcessState::Ready {
                proc.state = ProcessState::Blocked;
                proc.wait_channel = wait_channel;
            }
        }
    }

    pub fn unblock_process(&mut self, pid: u32) {
        if let Some(proc) = &mut self.processes[pid as usize] {
            if proc.state == ProcessState::Blocked {
                proc.state = ProcessState::Ready;
            }
        }
    }

    pub fn wake_process(&mut self, wait_channel: u32) {
        for i in 0..MAX_PROCESSES {
            if let Some(proc) = &mut self.processes[i] {
                if proc.state == ProcessState::Blocked && proc.wait_channel == wait_channel {
                    proc.state = ProcessState::Ready;
                }
            }
        }
    }

    pub fn schedule(&mut self) -> Option<u32> {
        let current = self.current_pid;

        if let Some(pid) = current {
            if let Some(proc) = &mut self.processes[pid as usize] {
                if proc.state == ProcessState::Running {
                    proc.state = ProcessState::Ready;
                    proc.time_remaining = proc.priority.time_slice();
                }
            }
        }

        let mut best_pid = None;
        let mut best_priority = ProcessPriority::Idle as u8;

        for i in 0..MAX_PROCESSES {
            if let Some(proc) = &self.processes[i] {
                if proc.state == ProcessState::Ready {
                    let pri_val = proc.priority as u8;
                    if pri_val < best_priority {
                        best_priority = pri_val;
                        best_pid = Some(proc.pid);
                    }
                }
            }
        }

        if let Some(pid) = best_pid {
            if let Some(proc) = &mut self.processes[pid as usize] {
                proc.state = ProcessState::Running;
                proc.time_remaining = proc.priority.time_slice();
            }
            self.current_pid = Some(pid);
            self.context_switches += 1;
        }

        self.current_pid
    }

    pub fn tick(&mut self) -> Option<u32> {
        self.tick_count += 1;

        if let Some(pid) = self.current_pid {
            if let Some(proc) = &mut self.processes[pid as usize] {
                if proc.state == ProcessState::Running {
                    proc.time_remaining = proc.time_remaining.saturating_sub(1);
                    proc.total_time += 1;

                    if proc.time_remaining == 0 {
                        return self.schedule();
                    }
                }
            }
        }

        self.current_pid
    }

    pub fn get_current_pid(&self) -> Option<u32> {
        self.current_pid
    }

    pub fn get_current_process(&self) -> Option<&Process> {
        self.current_pid.and_then(|pid| self.get_process(pid))
    }

    pub fn get_current_process_mut(&mut self) -> Option<&mut Process> {
        self.current_pid.and_then(|pid| self.get_process_mut(pid))
    }

    pub fn get_running_count(&self) -> usize {
        self.processes
            .iter()
            .filter(|p| p.as_ref().map_or(false, |proc| proc.is_runnable()))
            .count()
    }

    pub fn get_all_processes(&self) -> Vec<u32> {
        self.processes
            .iter()
            .filter_map(|p| p.as_ref().map(|proc| proc.pid))
            .collect()
    }

    pub fn get_children(&self, ppid: u32) -> Vec<u32> {
        self.processes
            .iter()
            .filter_map(|p| {
                p.as_ref()
                    .filter(|proc| proc.ppid == ppid)
                    .map(|proc| proc.pid)
            })
            .collect()
    }
}
