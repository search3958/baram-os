#![no_std]

extern crate alloc;

pub const SYS_EXIT: u64 = 0;
pub const SYS_FORK: u64 = 1;
pub const SYS_EXEC: u64 = 2;
pub const SYS_WAIT: u64 = 3;
pub const SYS_GETPID: u64 = 4;
pub const SYS_GETPPID: u64 = 5;
pub const SYS_YIELD: u64 = 6;
pub const SYS_SLEEP: u64 = 7;
pub const SYS_KILL: u64 = 8;

pub const SYS_MMAP: u64 = 10;
pub const SYS_MUNMAP: u64 = 11;
pub const SYS_MPROTECT: u64 = 12;
pub const SYS_BRK: u64 = 13;

pub const SYS_OPEN: u64 = 20;
pub const SYS_CLOSE: u64 = 21;
pub const SYS_READ: u64 = 22;
pub const SYS_WRITE: u64 = 23;
pub const SYS_LSEEK: u64 = 24;
pub const SYS_STAT: u64 = 25;

pub const SYS_PIPE: u64 = 30;
pub const SYS_DUP: u64 = 31;
pub const SYS_DUP2: u64 = 32;

pub const SYS_SOCKET: u64 = 40;
pub const SYS_BIND: u64 = 41;
pub const SYS_LISTEN: u64 = 42;
pub const SYS_ACCEPT: u64 = 43;
pub const SYS_CONNECT: u64 = 44;
pub const SYS_SEND: u64 = 45;
pub const SYS_RECV: u64 = 46;

pub const SYS_SCHED_SET_PRIORITY: u64 = 50;
pub const SYS_SCHED_GET_PRIORITY: u64 = 51;

pub const SYS_IPC_SEND: u64 = 60;
pub const SYS_IPC_RECV: u64 = 61;
pub const SYS_IPC_CALL: u64 = 62;
pub const SYS_IPC_REPLY: u64 = 63;

pub const SYS_GET_TIME: u64 = 70;
pub const SYS_SET_ALARM: u64 = 71;

pub const SYS_FB_MAP: u64 = 80;
pub const SYS_FB_UNMAP: u64 = 81;

pub struct SyscallResult {
    pub value: i64,
    pub error: i32,
}

impl SyscallResult {
    pub fn success(value: i64) -> Self {
        Self { value, error: 0 }
    }

    pub fn error(code: i32) -> Self {
        Self {
            value: -1,
            error: code,
        }
    }
}

pub fn handle_syscall(
    syscall_num: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
    _arg5: u64,
) -> SyscallResult {
    match syscall_num {
        SYS_EXIT => sys_exit(arg0 as i32),
        SYS_FORK => sys_fork(),
        SYS_EXEC => sys_exec(arg0, arg1),
        SYS_WAIT => sys_wait(arg0 as i32),
        SYS_GETPID => sys_getpid(),
        SYS_GETPPID => sys_getppid(),
        SYS_YIELD => sys_yield(),
        SYS_SLEEP => sys_sleep(arg0),
        SYS_KILL => sys_kill(arg0 as u32, arg1 as i32),

        SYS_MMAP => sys_mmap(arg0, arg1, arg2 as i32, arg3 as i32),
        SYS_MUNMAP => sys_munmap(arg0, arg1),
        SYS_MPROTECT => sys_mprotect(arg0, arg1, arg2 as i32),
        SYS_BRK => sys_brk(arg0),

        SYS_OPEN => sys_open(arg0, arg1 as u32),
        SYS_CLOSE => sys_close(arg0 as u32),
        SYS_READ => sys_read(arg0 as u32, arg1, arg2 as usize),
        SYS_WRITE => sys_write(arg0 as u32, arg1, arg2 as usize),
        SYS_LSEEK => sys_lseek(arg0 as u32, arg1 as i64, arg2 as i32),
        SYS_STAT => sys_stat(arg0, arg1),

        SYS_PIPE => sys_pipe(arg0),
        SYS_DUP => sys_dup(arg0 as u32),
        SYS_DUP2 => sys_dup2(arg0 as u32, arg1 as u32),

        SYS_SOCKET => sys_socket(arg0 as u32, arg1 as u32, arg2 as u32),
        SYS_BIND => sys_bind(arg0 as u32, arg1, arg2 as u32),
        SYS_LISTEN => sys_listen(arg0 as u32, arg1 as u32),
        SYS_ACCEPT => sys_accept(arg0 as u32, arg1, arg2),
        SYS_CONNECT => sys_connect(arg0 as u32, arg1, arg2 as u32),
        SYS_SEND => sys_send(arg0 as u32, arg1, arg2 as usize, arg3 as u32),
        SYS_RECV => sys_recv(arg0 as u32, arg1, arg2 as usize, arg3 as u32),

        SYS_SCHED_SET_PRIORITY => sys_set_priority(arg0 as u32, arg1 as u8),
        SYS_SCHED_GET_PRIORITY => sys_get_priority(arg0 as u32),

        SYS_IPC_SEND => sys_ipc_send(arg0 as u32, arg1, arg2 as usize),
        SYS_IPC_RECV => sys_ipc_recv(arg0, arg1 as usize),
        SYS_IPC_CALL => sys_ipc_call(arg0 as u32, arg1, arg2 as usize),
        SYS_IPC_REPLY => sys_ipc_reply(arg0 as u32, arg1, arg2 as usize),

        SYS_GET_TIME => sys_get_time(),
        SYS_SET_ALARM => sys_set_alarm(arg0),

        SYS_FB_MAP => sys_fb_map(arg0, arg1, arg2),
        SYS_FB_UNMAP => sys_fb_unmap(arg0, arg1),

        _ => SyscallResult::error(-1),
    }
}

fn sys_exit(_code: i32) -> SyscallResult {
    SyscallResult::success(0)
}

fn sys_fork() -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_exec(_path: u64, _args: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_wait(_pid: i32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_getpid() -> SyscallResult {
    SyscallResult::success(0)
}

fn sys_getppid() -> SyscallResult {
    SyscallResult::success(0)
}

fn sys_yield() -> SyscallResult {
    SyscallResult::success(0)
}

fn sys_sleep(_ms: u64) -> SyscallResult {
    SyscallResult::success(0)
}

fn sys_kill(_pid: u32, _sig: i32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_mmap(_addr: u64, _len: u64, _prot: i32, _flags: i32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_munmap(_addr: u64, _len: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_mprotect(_addr: u64, _len: u64, _prot: i32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_brk(_addr: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_open(_path: u64, _flags: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_close(_fd: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_read(_fd: u32, _buf: u64, _len: usize) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_write(_fd: u32, _buf: u64, _len: usize) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_lseek(_fd: u32, _offset: i64, _whence: i32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_stat(_path: u64, _buf: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_pipe(_pipefd: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_dup(_fd: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_dup2(_oldfd: u32, _newfd: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_socket(_domain: u32, _type: u32, _protocol: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_bind(_sockfd: u32, _addr: u64, _addrlen: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_listen(_sockfd: u32, _backlog: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_accept(_sockfd: u32, _addr: u64, _addrlen: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_connect(_sockfd: u32, _addr: u64, _addrlen: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_send(_sockfd: u32, _buf: u64, _len: usize, _flags: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_recv(_sockfd: u32, _buf: u64, _len: usize, _flags: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_set_priority(_pid: u32, _priority: u8) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_get_priority(_pid: u32) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_ipc_send(_dest: u32, _msg: u64, _len: usize) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_ipc_recv(_msg: u64, _len: usize) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_ipc_call(_dest: u32, _msg: u64, _len: usize) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_ipc_reply(_src: u32, _msg: u64, _len: usize) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_get_time() -> SyscallResult {
    SyscallResult::success(0)
}

fn sys_set_alarm(_ms: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_fb_map(_width: u64, _height: u64, _format: u64) -> SyscallResult {
    SyscallResult::error(-1)
}

fn sys_fb_unmap(_addr: u64, _size: u64) -> SyscallResult {
    SyscallResult::error(-1)
}
