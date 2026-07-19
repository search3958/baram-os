#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use crate::loader::{LoadedModule, load_pe_from_memory, find_exports};
pub use baram_core::subsystem::{SubsystemExports, SubsystemContext, KeyEventData, MouseEventData};

pub struct SubsystemManager {
    subsystems: Vec<LoadedSubsystem>,
}

struct LoadedSubsystem {
    module: LoadedModule,
    exports: *const SubsystemExports,
    context: SubsystemContext,
    initialized: bool,
}

unsafe impl Send for SubsystemManager {}

impl SubsystemManager {
    pub fn new() -> Self {
        Self {
            subsystems: Vec::new(),
        }
    }

    pub fn load_subsystem(&mut self, data: &[u8]) -> Result<usize, crate::loader::LoadError> {
        let module = load_pe_from_memory(data)?;

        let exports = find_exports(&module)
            .ok_or(crate::loader::LoadError::InvalidFormat)?;

        let context = SubsystemContext {
            magic: baram_core::subsystem::SUBSYSTEM_MAGIC,
            version: baram_core::subsystem::SUBSYSTEM_VERSION,
            fb: baram_core::subsystem::FramebufferInfo {
                base: core::ptr::null_mut(),
                width: 0,
                height: 0,
            },
            mouse_x: 0,
            mouse_y: 0,
            screen_w: 0,
            screen_h: 0,
            tick_count: 0,
            fps: 0,
            userdata: core::ptr::null_mut(),
        };

        let idx = self.subsystems.len();
        self.subsystems.push(LoadedSubsystem {
            module,
            exports,
            context,
            initialized: false,
        });

        Ok(idx)
    }

    pub fn init_subsystem(&mut self, idx: usize, fb: &mut [u32], width: u32, height: u32) -> i32 {
        if idx >= self.subsystems.len() {
            return -1;
        }

        let sub = &mut self.subsystems[idx];
        sub.context.fb = baram_core::subsystem::FramebufferInfo {
            base: fb.as_mut_ptr(),
            width,
            height,
        };
        sub.context.screen_w = width;
        sub.context.screen_h = height;

        let exports = unsafe { &*sub.exports };
        let result = (exports.init)(&mut sub.context as *mut SubsystemContext);

        if result == 0 {
            sub.initialized = true;
        }

        result
    }

    pub fn handle_key(&mut self, idx: usize, event: &KeyEventData) -> i32 {
        if idx >= self.subsystems.len() || !self.subsystems[idx].initialized {
            return -1;
        }

        let sub = &mut self.subsystems[idx];
        let exports = unsafe { &*sub.exports };
        (exports.handle_key)(&mut sub.context as *mut SubsystemContext, event as *const KeyEventData)
    }

    pub fn handle_mouse(&mut self, idx: usize, event: &MouseEventData) -> i32 {
        if idx >= self.subsystems.len() || !self.subsystems[idx].initialized {
            return -1;
        }

        let sub = &mut self.subsystems[idx];
        let exports = unsafe { &*sub.exports };
        (exports.handle_mouse)(&mut sub.context as *mut SubsystemContext, event as *const MouseEventData)
    }

    pub fn tick(&mut self, idx: usize) -> i32 {
        if idx >= self.subsystems.len() || !self.subsystems[idx].initialized {
            return -1;
        }

        let sub = &mut self.subsystems[idx];
        sub.context.tick_count += 1;
        let exports = unsafe { &*sub.exports };
        (exports.tick)(&mut sub.context as *mut SubsystemContext)
    }

    pub fn render(&mut self, idx: usize) -> i32 {
        if idx >= self.subsystems.len() || !self.subsystems[idx].initialized {
            return -1;
        }

        let sub = &mut self.subsystems[idx];
        let exports = unsafe { &*sub.exports };
        (exports.render)(&mut sub.context as *mut SubsystemContext)
    }

    pub fn shutdown_all(&mut self) {
        for sub in &mut self.subsystems {
            if sub.initialized {
                let exports = unsafe { &*sub.exports };
                (exports.shutdown)(&mut sub.context as *mut SubsystemContext);
                sub.initialized = false;
            }
        }
    }

    pub fn count(&self) -> usize {
        self.subsystems.len()
    }

    pub fn get_context(&self, idx: usize) -> Option<&SubsystemContext> {
        if idx < self.subsystems.len() {
            Some(&self.subsystems[idx].context)
        } else {
            None
        }
    }

    pub fn get_context_mut(&mut self, idx: usize) -> Option<&mut SubsystemContext> {
        if idx < self.subsystems.len() {
            Some(&mut self.subsystems[idx].context)
        } else {
            None
        }
    }
}
