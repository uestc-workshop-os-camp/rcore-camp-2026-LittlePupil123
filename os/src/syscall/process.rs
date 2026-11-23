//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next,TASK_MANAGER},
    timer::get_time_us,
};



#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();//task子模块下的该函数，功能如字面意思
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// TODO: implement the syscall
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    match trace_request{
        0 => {
            let id_ptr = id as *const u8;
            let value = unsafe{id_ptr.read_volatile()};
            value as isize
        },
        1 => {
            let id_ptr = id as *mut u8;
            let value = data as u8;
            unsafe{id_ptr.write_volatile(value)};
            0
        },
        2 => {
            let times={
                let task_inner = TASK_MANAGER.get_inner();
                let current = task_inner.current_task;
                if id<500{
                    task_inner.tasks[current].syscall_times[id]
                }else{ 0 }
            };
            times as isize
        },
        _=> {
            -1
        },
    }    
}
