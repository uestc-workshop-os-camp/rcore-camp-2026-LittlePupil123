//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next};

use crate::mm::translated_byte_buffer;
use crate::task::current_user_token; // 获取当前任务的页表 Token
use crate::timer::get_time;
use crate::task::TASK_MANAGER;
use crate::mm::MapPermission;
use crate::config::PAGE_SIZE;
use crate::mm::address::VirtAddr;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
/// 失效的原因在于：传入的_ts是一种用户态的指针，它是该应用地址空间下的某个虚拟起始地址，
/// 我们要做的就是将它转换为实际所在的物理地址，并可以在内核态时使用这块内存。
/// 由于用户态的虚拟地址可能不连续，甚至可能跨页，
/// 因此我们需要将其翻译为实际的物理地址，并确保我们可以安全地访问它们。
/// 通过使用`translated_byte_buffer`函数，
/// 我们可以将用户态的虚拟地址转换为内核态可访问的字节切片，
/// 从而实现对用户空间数据的安全访问和操作。
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");

    let token = current_user_token();
    // 获取 TimeVal 的大小（通常是 16 字节）
    let len = core::mem::size_of::<TimeVal>(); 
    // 把用户指针翻译成物理内存的可写字节切片
    //buffers是一系列字节切片，表示用户空间中的缓存所在的一系列物理页帧
    let mut buffers = translated_byte_buffer(token, ts as *mut u8, len);
    // 获取当前时间
    let time = get_time(); // 假设你有一个获取时间的函数返回 TimeVal
    
    // 将获取到的TimeVal结构体退化为字节切片的形式，以便可以安全地拷贝到用户空间
    let time_bytes = unsafe {
        core::slice::from_raw_parts(&time as *const _ as *const u8, len)
    };

    // 安全地拷贝到用户空间
    let mut copied = 0;
    for buffer in &mut buffers {//逐页将time_bytes拷贝进buffer中
        let copy_len = buffer.len().min(len - copied);
        buffer[..copy_len].copy_from_slice(&time_bytes[copied..copied + copy_len]);
        copied += copy_len;
    }
    
    len as isize // 返回写入的字节数，或者 0 表示成功

}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token();
    match trace_request{
        0 => {
            let buffers = translated_byte_buffer(token, id as *mut u8, 1);
            if buffers.is_empty() {
                return -1;
            }
            buffers[0][0] as isize
        },
        1 => {
            let mut buffers = translated_byte_buffer(token, id as *mut u8, 1);
            if buffers.is_empty() {
                return -1;
            }
            buffers[0][0] = data as u8;
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

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");

    //特判start是否按页对齐以及其他特判错误
    if start % PAGE_SIZE != 0 || port & !0x7 == 0 || port & 0x7 != 0{
        return -1;
    }
    
    if len == 0 { return 0; } //长度为0直接返回0，表示成功但不分配内存

    let start_vpn = start.into(); //将起始虚拟地址转换为VirtAddr类型
    let end_vpn =(start + len + PAGE_SIZE - 1) / PAGE_SIZE * PAGE_SIZE;//计算结束虚拟页号边界的虚拟地址

    
    //获取当前任务的地址空间
    let inner = TASK_MANAGER.get_inner();
    let current = inner.current_task;
    let memory_set = &mut inner.tasks[current].memory_set;

    // 判断区间是否重叠
    for area in memory_set.areas.iter() {
        let area_start = area.vpn_range.get_start().0;
        let area_end = area.vpn_range.get_end().0;
        
        // 核心重叠判断逻辑：
        // 请求的 start < 已有段的 end，并且 已有段的 start < 请求的 end
        if start_vpn < area_end && area_start < end_vpn {
            return -1; // 发生地址冲突，重叠了！
        }
    }
    
    // 将port转换为MapPermission
    let mut map_perm = MapPermission::U;
    if port & 1 != 0 { map_perm |= MapPermission::R; }
    if port & 2 != 0 { map_perm |= MapPermission::W; }
    if port & 4 != 0 { map_perm |= MapPermission::X; }

    memory_set.insert_framed_area(
        VirtAddr(start_vpn),
        VirtAddr(end_vpn),
        map_perm,
    );


    return 0;

}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    -1
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
