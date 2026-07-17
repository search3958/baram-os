#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub const MAX_PROCESSES: usize = 64;
pub const MAX_NAME_LEN: usize = 32;
pub const STACK_SIZE: usize = 0x10000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessState {
    Created,
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessPriority {
    Realtime,
    High,
    Normal,
    Low,
    Idle,
}

impl ProcessPriority {
    pub fn time_slice(&self) -> u32 {
        match self {
            ProcessPriority::Realtime => 100,
            ProcessPriority::High => 50,
            ProcessPriority::Normal => 25,
            ProcessPriority::Low => 10,
            ProcessPriority::Idle => 5,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CpuContext {
    pub x0: u64,
    pub x1: u64,
    pub x2: u64,
    pub x3: u64,
    pub x4: u64,
    pub x5: u64,
    pub x6: u64,
    pub x7: u64,
    pub x8: u64,
    pub x9: u64,
    pub x10: u64,
    pub x11: u64,
    pub x12: u64,
    pub x13: u64,
    pub x14: u64,
    pub x15: u64,
    pub x16: u64,
    pub x17: u64,
    pub x18: u64,
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64,
    pub lr: u64,
    pub sp: u64,
    pub elr_el1: u64,
    pub spsr_el1: u64,
    pub tcr_el1: u64,
    pub ttbr0_el1: u64,
    pub ttbr1_el1: u64,
}

impl CpuContext {
    pub fn new() -> Self {
        Self {
            x0: 0, x1: 0, x2: 0, x3: 0, x4: 0, x5: 0, x6: 0, x7: 0,
            x8: 0, x9: 0, x10: 0, x11: 0, x12: 0, x13: 0, x14: 0, x15: 0,
            x16: 0, x17: 0, x18: 0, x19: 0, x20: 0, x21: 0, x22: 0, x23: 0,
            x24: 0, x25: 0, x26: 0, x27: 0, x28: 0, x29: 0,
            lr: 0, sp: 0, elr_el1: 0, spsr_el1: 0,
            tcr_el1: 0, ttbr0_el1: 0, ttbr1_el1: 0,
        }
    }
}

pub struct Process {
    pub pid: u32,
    pub ppid: u32,
    pub name: [u8; MAX_NAME_LEN],
    pub name_len: usize,
    pub state: ProcessState,
    pub priority: ProcessPriority,
    pub context: CpuContext,
    pub kernel_stack: usize,
    pub user_stack: usize,
    pub stack_size: usize,
    pub page_table: usize,
    pub time_remaining: u32,
    pub total_time: u64,
    pub wait_channel: u32,
    pub exit_code: i32,
    pub children: Vec<u32>,
    pub fd_table: [usize; 16],
}

impl Process {
    pub fn new(pid: u32, name: &str, priority: ProcessPriority) -> Self {
        let mut proc_name = [0u8; MAX_NAME_LEN];
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(MAX_NAME_LEN);
        proc_name[..name_len].copy_from_slice(&name_bytes[..name_len]);

        Self {
            pid,
            ppid: 0,
            name: proc_name,
            name_len,
            state: ProcessState::Created,
            priority,
            context: CpuContext::new(),
            kernel_stack: 0,
            user_stack: 0,
            stack_size: STACK_SIZE,
            page_table: 0,
            time_remaining: priority.time_slice(),
            total_time: 0,
            wait_channel: 0,
            exit_code: 0,
            children: Vec::new(),
            fd_table: [0; 16],
        }
    }

    pub fn get_name(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.name[..self.name_len]) }
    }

    pub fn is_runnable(&self) -> bool {
        self.state == ProcessState::Ready || self.state == ProcessState::Running
    }
}
