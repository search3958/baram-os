#![no_std]

extern crate alloc;

use crate::ipc::IpcManager;
use crate::proc_loader::ProcessLoader;
use crate::process::{ProcessPriority, ProcessState};
use crate::scheduler::Scheduler;
use crate::vmm::VirtualMemoryManager;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const INIT_PATH: &str = "/bin/init";
pub const SYSTEM_SERVICES: &[&str] = &[
    "/bin/windowserver",
    "/bin/inputd",
    "/bin/netd",
    "/bin/fswatch",
];

pub struct InitProcess {
    pub pid: u32,
    pub state: InitState,
    pub services: Vec<ServiceInfo>,
    pub environment: Vec<(String, String)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InitState {
    NotStarted,
    Loading,
    Running,
    Failed,
}

#[derive(Clone)]
pub struct ServiceInfo {
    pub name: String,
    pub path: String,
    pub pid: Option<u32>,
    pub state: ServiceState,
    pub auto_restart: bool,
    pub restart_count: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    NotStarted,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

impl InitProcess {
    pub fn new() -> Self {
        Self {
            pid: 0,
            state: InitState::NotStarted,
            services: Vec::new(),
            environment: Vec::new(),
        }
    }

    pub fn start(
        &mut self,
        loader: &mut ProcessLoader,
        scheduler: &mut Scheduler,
        vmm: &mut VirtualMemoryManager,
    ) -> Result<(), InitError> {
        self.state = InitState::Loading;

        let (process, _load_info) = loader
            .load_executable(INIT_PATH, vmm)
            .map_err(|_| InitError::LoadFailed)?;

        self.pid = process.pid;

        scheduler.create_process(INIT_PATH, ProcessPriority::High);

        self.setup_environment();

        for service_path in SYSTEM_SERVICES {
            self.add_service(service_path);
        }

        self.state = InitState::Running;

        Ok(())
    }

    pub fn tick(&mut self, scheduler: &mut Scheduler) {
        if self.state != InitState::Running {
            return;
        }

        for service in &mut self.services {
            match service.state {
                ServiceState::NotStarted => {
                    service.state = ServiceState::Starting;
                }
                ServiceState::Starting => {
                    if let Some(pid) = service.pid {
                        if let Some(proc) = scheduler.get_process(pid) {
                            if proc.state == ProcessState::Running {
                                service.state = ServiceState::Running;
                            }
                        }
                    }
                }
                ServiceState::Running => {
                    if let Some(pid) = service.pid {
                        if let Some(proc) = scheduler.get_process(pid) {
                            if proc.state == ProcessState::Terminated {
                                service.state = ServiceState::Stopped;
                            }
                        }
                    }
                }
                ServiceState::Stopped => {
                    if service.auto_restart && service.restart_count < 3 {
                        service.state = ServiceState::NotStarted;
                        service.restart_count += 1;
                    }
                }
                ServiceState::Stopping => {
                    if let Some(pid) = service.pid {
                        scheduler.terminate_process(pid, 0);
                        service.state = ServiceState::Stopped;
                    }
                }
                ServiceState::Failed => {
                    if service.auto_restart && service.restart_count < 3 {
                        service.state = ServiceState::NotStarted;
                        service.restart_count += 1;
                    }
                }
            }
        }
    }

    pub fn shutdown(&mut self, _scheduler: &mut Scheduler) {
        for service in self.services.iter_mut().rev() {
            if service.state == ServiceState::Running {
                service.state = ServiceState::Stopping;
            }
        }

        self.state = InitState::NotStarted;
    }

    fn setup_environment(&mut self) {
        self.environment.push((
            String::from("PATH"),
            String::from("/bin:/usr/bin:/system/bin"),
        ));
        self.environment
            .push((String::from("HOME"), String::from("/root")));
        self.environment
            .push((String::from("USER"), String::from("root")));
        self.environment
            .push((String::from("SHELL"), String::from("/bin/sh")));
        self.environment
            .push((String::from("TERM"), String::from("xterm-256color")));
        self.environment
            .push((String::from("LANG"), String::from("C")));
        self.environment
            .push((String::from("TZ"), String::from("UTC")));
    }

    fn add_service(&mut self, path: &str) {
        let name = path.split('/').last().unwrap_or(path).to_string();

        let service = ServiceInfo {
            name,
            path: path.to_string(),
            pid: None,
            state: ServiceState::NotStarted,
            auto_restart: true,
            restart_count: 0,
        };

        self.services.push(service);
    }

    pub fn get_service(&self, name: &str) -> Option<&ServiceInfo> {
        self.services.iter().find(|s| s.name == name)
    }

    pub fn get_service_mut(&mut self, name: &str) -> Option<&mut ServiceInfo> {
        self.services.iter_mut().find(|s| s.name == name)
    }

    pub fn get_services(&self) -> &[ServiceInfo] {
        &self.services
    }

    pub fn get_running_services(&self) -> Vec<&ServiceInfo> {
        self.services
            .iter()
            .filter(|s| s.state == ServiceState::Running)
            .collect()
    }

    pub fn get_environment(&self) -> &[(String, String)] {
        &self.environment
    }

    pub fn set_environment(&mut self, key: &str, value: &str) {
        if let Some(entry) = self.environment.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value.to_string();
        } else {
            self.environment.push((key.to_string(), value.to_string()));
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == InitState::Running
    }

    pub fn get_pid(&self) -> u32 {
        self.pid
    }
}

pub struct SystemInit {
    init: InitProcess,
    loader: ProcessLoader,
    scheduler: Scheduler,
    vmm: VirtualMemoryManager,
    ipc: IpcManager,
}

impl SystemInit {
    pub fn new() -> Self {
        Self {
            init: InitProcess::new(),
            loader: ProcessLoader::new(),
            scheduler: Scheduler::new(),
            vmm: VirtualMemoryManager::new(),
            ipc: IpcManager::new(),
        }
    }

    pub fn boot(&mut self) -> Result<(), InitError> {
        self.vmm.init(0, 0);

        self.init
            .start(&mut self.loader, &mut self.scheduler, &mut self.vmm)?;

        self.start_services()?;

        Ok(())
    }

    fn start_services(&mut self) -> Result<(), InitError> {
        for service in &mut self.init.services {
            if service.state == ServiceState::NotStarted {
                let (process, _load_info) = self
                    .loader
                    .load_executable(&service.path, &mut self.vmm)
                    .map_err(|_| InitError::ServiceLoadFailed)?;

                service.pid = Some(process.pid);
                service.state = ServiceState::Starting;

                self.scheduler
                    .create_process(&service.path, ProcessPriority::Normal);
            }
        }

        Ok(())
    }

    pub fn tick(&mut self) {
        self.init.tick(&mut self.scheduler);
        self.scheduler.tick();
    }

    pub fn shutdown(&mut self) {
        self.init.shutdown(&mut self.scheduler);
    }

    pub fn get_scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    pub fn get_scheduler_mut(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    pub fn get_vmm(&self) -> &VirtualMemoryManager {
        &self.vmm
    }

    pub fn get_vmm_mut(&mut self) -> &mut VirtualMemoryManager {
        &mut self.vmm
    }

    pub fn get_ipc(&self) -> &IpcManager {
        &self.ipc
    }

    pub fn get_ipc_mut(&mut self) -> &mut IpcManager {
        &mut self.ipc
    }

    pub fn get_init(&self) -> &InitProcess {
        &self.init
    }

    pub fn get_loader(&self) -> &ProcessLoader {
        &self.loader
    }

    pub fn get_loader_mut(&mut self) -> &mut ProcessLoader {
        &mut self.loader
    }
}

#[derive(Debug)]
pub enum InitError {
    LoadFailed,
    ServiceLoadFailed,
    SchedulerError,
    MemoryError,
}
