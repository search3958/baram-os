#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use alloc::boxed::Box;

pub const MAX_IPC_PORTS: usize = 256;
pub const MAX_IPC_MESSAGES: usize = 64;
pub const MAX_IPC_MESSAGE_SIZE: usize = 4096;

pub const IPC_PORT權利_ALL: u32 = 0xFFFFFFFF;
pub const IPC_PORT權利_READ: u32 = 1;
pub const IPC_PORT權利_WRITE: u32 = 2;
pub const IPC_PORT權利_READ_WRITE: u32 = 3;

pub const IPC_MSG_TYPE_NORMAL: u32 = 0;
pub const IPC_MSG_TYPE_NOTIFY: u32 = 1;
pub const IPC_MSG_TYPE_OOL: u32 = 2;

pub const IPC_SEND_TIMEOUT: u64 = 1000;
pub const IPC_RECV_TIMEOUT: u64 = 1000;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IpcHeader {
    pub msg_type: u32,
    pub msg_id: u32,
    pub msg_size: u32,
    pub msg_flags: u32,
    pub msg_remote_port: u32,
    pub msg_local_port: u32,
    pub msg_reserved: u32,
}

#[repr(C)]
#[derive(Clone)]
pub struct IpcMessage {
    pub header: IpcHeader,
    pub data: Vec<u8>,
    pub ool_data: Vec<u8>,
}

impl IpcMessage {
    pub fn new(msg_type: u32, msg_id: u32) -> Self {
        Self {
            header: IpcHeader {
                msg_type,
                msg_id,
                msg_size: 0,
                msg_flags: 0,
                msg_remote_port: 0,
                msg_local_port: 0,
                msg_reserved: 0,
            },
            data: Vec::new(),
            ool_data: Vec::new(),
        }
    }

    pub fn with_data(msg_type: u32, msg_id: u32, data: Vec<u8>) -> Self {
        let size = data.len() as u32;
        Self {
            header: IpcHeader {
                msg_type,
                msg_id,
                msg_size: size,
                msg_flags: 0,
                msg_remote_port: 0,
                msg_local_port: 0,
                msg_reserved: 0,
            },
            data,
            ool_data: Vec::new(),
        }
    }

    pub fn set_remote_port(&mut self, port: u32) {
        self.header.msg_remote_port = port;
    }

    pub fn set_local_port(&mut self, port: u32) {
        self.header.msg_local_port = port;
    }
}

pub struct IpcPort {
    pub port_id: u32,
    pub name: u64,
    pub owner_pid: u32,
    pub rights: u32,
    pub message_queue: Vec<IpcMessage>,
    pub waiting_receivers: Vec<u32>,
    pub waiting_senders: Vec<u32>,
    pub is_dynamic: bool,
}

pub struct IpcManager {
    ports: Vec<Option<IpcPort>>,
    next_port_id: u32,
    port_names: BTreeMap<u64, u32>,
    pending_messages: Vec<(u32, IpcMessage)>,
}

impl IpcManager {
    pub fn new() -> Self {
        let mut ports = Vec::with_capacity(MAX_IPC_PORTS);
        for _ in 0..MAX_IPC_PORTS {
            ports.push(None);
        }

        Self {
            ports,
            next_port_id: 1,
            port_names: BTreeMap::new(),
            pending_messages: Vec::new(),
        }
    }

    pub fn create_port(&mut self, name: u64, owner_pid: u32, rights: u32) -> Option<u32> {
        if self.port_names.contains_key(&name) {
            return None;
        }

        let port_id = self.next_port_id;
        self.next_port_id += 1;

        if port_id as usize >= MAX_IPC_PORTS {
            return None;
        }

        let port = IpcPort {
            port_id,
            name,
            owner_pid,
            rights,
            message_queue: Vec::new(),
            waiting_receivers: Vec::new(),
            waiting_senders: Vec::new(),
            is_dynamic: false,
        };

        self.ports[port_id as usize] = Some(port);
        self.port_names.insert(name, port_id);

        Some(port_id)
    }

    pub fn create_dynamic_port(&mut self, owner_pid: u32, rights: u32) -> Option<u32> {
        let port_id = self.next_port_id;
        self.next_port_id += 1;

        if port_id as usize >= MAX_IPC_PORTS {
            return None;
        }

        let port = IpcPort {
            port_id,
            name: port_id as u64,
            owner_pid,
            rights,
            message_queue: Vec::new(),
            waiting_receivers: Vec::new(),
            waiting_senders: Vec::new(),
            is_dynamic: true,
        };

        self.ports[port_id as usize] = Some(port);

        Some(port_id)
    }

    pub fn destroy_port(&mut self, port_id: u32) -> bool {
        if port_id as usize >= MAX_IPC_PORTS {
            return false;
        }

        if let Some(port) = &self.ports[port_id as usize] {
            self.port_names.remove(&port.name);
        }

        self.ports[port_id as usize] = None;
        true
    }

    pub fn lookup_port(&self, name: u64) -> Option<u32> {
        self.port_names.get(&name).copied()
    }

    pub fn send_message(&mut self, port_id: u32, message: IpcMessage, timeout: u64) -> Result<(), IpcError> {
        if port_id as usize >= MAX_IPC_PORTS {
            return Err(IpcError::InvalidPort);
        }

        let port = self.ports[port_id as usize].as_mut()
            .ok_or(IpcError::InvalidPort)?;

        if !port.rights & IPC_PORT權利_WRITE != 0 && port.rights != IPC_PORT權利_ALL {
            return Err(IpcError::PermissionDenied);
        }

        if let Some(waiting_pid) = port.waiting_receivers.pop() {
            self.pending_messages.push((waiting_pid, message));
            return Ok(());
        }

        if port.message_queue.len() >= MAX_IPC_MESSAGES {
            if timeout == 0 {
                return Err(IpcError::QueueFull);
            }
            port.waiting_senders.push(0);
            return Ok(());
        }

        port.message_queue.push(message);
        Ok(())
    }

    pub fn receive_message(&mut self, port_id: u32, timeout: u64) -> Result<IpcMessage, IpcError> {
        if port_id as usize >= MAX_IPC_PORTS {
            return Err(IpcError::InvalidPort);
        }

        let port = self.ports[port_id as usize].as_mut()
            .ok_or(IpcError::InvalidPort)?;

        if !port.rights & IPC_PORT權利_READ != 0 && port.rights != IPC_PORT權利_ALL {
            return Err(IpcError::PermissionDenied);
        }

        if let Some(msg) = port.message_queue.pop() {
            return Ok(msg);
        }

        if timeout == 0 {
            return Err(IpcError::NoMessage);
        }

        port.waiting_receivers.push(0);
        Err(IpcError::WouldBlock)
    }

    pub fn receive_for_pid(&mut self, pid: u32) -> Option<IpcMessage> {
        self.pending_messages.iter()
            .position(|(p, _)| *p == pid)
            .map(|i| self.pending_messages.remove(i).1)
    }

    pub fn portable_copy_right(&mut self, src_port: u32, dst_port: u32) -> Result<(), IpcError> {
        if src_port as usize >= MAX_IPC_PORTS || dst_port as usize >= MAX_IPC_PORTS {
            return Err(IpcError::InvalidPort);
        }

        let src = self.ports[src_port as usize].as_ref()
            .ok_or(IpcError::InvalidPort)?;
        let dst_rights = src.rights;

        if let Some(dst) = &mut self.ports[dst_port as usize] {
            dst.rights |= dst_rights;
        } else {
            return Err(IpcError::InvalidPort);
        }

        Ok(())
    }

    pub fn get_port_info(&self, port_id: u32) -> Option<&IpcPort> {
        self.ports.get(port_id as usize)?.as_ref()
    }

    pub fn get_port_info_mut(&mut self, port_id: u32) -> Option<&mut IpcPort> {
        self.ports.get_mut(port_id as usize)?.as_mut()
    }

    pub fn get_ports_for_pid(&self, pid: u32) -> Vec<u32> {
        self.ports.iter()
            .filter_map(|p| p.as_ref().filter(|port| port.owner_pid == pid).map(|port| port.port_id))
            .collect()
    }
}

#[derive(Debug)]
pub enum IpcError {
    InvalidPort,
    PermissionDenied,
    QueueFull,
    NoMessage,
    WouldBlock,
    Timeout,
    InvalidMessage,
    OutOfMemory,
}

pub struct IpcClient {
    local_port: u32,
    remote_port: u32,
    msg_id: u32,
}

impl IpcClient {
    pub fn new(local_port: u32, remote_port: u32) -> Self {
        Self {
            local_port,
            remote_port,
            msg_id: 0,
        }
    }

    pub fn send(&mut self, data: Vec<u8>) -> Result<(), IpcError> {
        let mut msg = IpcMessage::with_data(IPC_MSG_TYPE_NORMAL, self.msg_id, data);
        msg.set_remote_port(self.remote_port);
        msg.set_local_port(self.local_port);

        self.msg_id += 1;

        Err(IpcError::WouldBlock)
    }

    pub fn receive(&self) -> Result<IpcMessage, IpcError> {
        Err(IpcError::WouldBlock)
    }
}

pub struct IpcServer {
    port_id: u32,
    handler: Option<Box<dyn FnMut(IpcMessage) -> IpcMessage>>,
}

impl IpcServer {
    pub fn new(port_id: u32) -> Self {
        Self {
            port_id,
            handler: None,
        }
    }

    pub fn set_handler(&mut self, handler: Box<dyn FnMut(IpcMessage) -> IpcMessage>) {
        self.handler = Some(handler);
    }

    pub fn handle_message(&mut self, message: IpcMessage) -> IpcMessage {
        if let Some(handler) = &mut self.handler {
            handler(message)
        } else {
            let mut reply = IpcMessage::new(IPC_MSG_TYPE_NORMAL, message.header.msg_id);
            reply.set_remote_port(message.header.msg_local_port);
            reply.set_local_port(message.header.msg_remote_port);
            reply
        }
    }
}
