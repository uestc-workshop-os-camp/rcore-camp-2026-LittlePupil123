//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next};

use crate::config::PAGE_SIZE;
use crate::mm::{translated_byte_buffer, MapPermission, VirtAddr};
use crate::task::{current_user_token, TASK_MANAGER};
use crate::timer::get_time;

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
        2 => TASK_MANAGER.syscall_times(id) as isize,
        _=> {
            -1
        },
    }
}

// YOUR JOB: Implement mmap.
//
// 简化版 mmap：只负责分配内存，不做文件映射。
// - `start` 必须按页对齐，否则返回 -1；
// - `len` 不要求对齐，会向上取整到页边界；len 为 0 视为空操作，直接返回 0；
// - `prot` 的低三位依次表示读(R)、写(W)、执行(X)权限，且不能为 0；
// - 成功返回 0，失败返回 -1。
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap");

    // start 必须对齐到页边界；prot 只能使用低 3 位且不能为 0
    if start % PAGE_SIZE != 0 || prot == 0 || prot & !0x7 != 0 {
        return -1;
    }
    // 长度为 0 不分配任何内存，直接返回成功
    if len == 0 {
        return 0;
    }

    // 结束地址向上取整到页边界：即使 len 不是页的整数倍，也要覆盖到最后一页
    let end = match start.checked_add(len) {
        Some(v) => (v + PAGE_SIZE - 1) & !(PAGE_SIZE - 1),
        None => return -1,
    };
    if end <= start {
        return -1;
    }

    // prot 的低三位依次表示读、写、执行权限，另外始终带上 U（用户态可访问）
    let mut map_perm = MapPermission::U;
    if prot & 0x1 != 0 {
        map_perm |= MapPermission::R;
    }
    if prot & 0x2 != 0 {
        map_perm |= MapPermission::W;
    }
    if prot & 0x4 != 0 {
        map_perm |= MapPermission::X;
    }

    // 通过 TaskManager 的方法拿到当前任务的地址空间，不直接接触 TaskManagerInner
    TASK_MANAGER.with_current_memory_set(|memory_set| {
        let start_va: VirtAddr = start.into();
        let end_va: VirtAddr = end.into();
        // 与已有逻辑段重叠则拒绝，避免覆盖已有的映射
        if !memory_set.is_range_free(start_va, end_va) {
            return -1;
        }
        memory_set.insert_framed_area(start_va, end_va, map_perm);
        // 无需调用 activate()：陷入内核时 satp 已切到内核页表，返回用户态时
        // __restore 会写回用户 satp 并执行 sfence.vma，新映射自然生效。
        0
    })
}

// YOUR JOB: Implement munmap.
//
// 简化版 munmap：解除 [start, start + len) 内的页映射并释放物理页帧。
// start 和 len 都必须按页对齐，否则返回 -1；
// 区间内存在未映射的页（即区间不连续）也返回 -1；
// len 为 0 视为空操作，直接返回 0。
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");

    if start % PAGE_SIZE != 0 || len % PAGE_SIZE != 0 {//判断是否按页对齐start
        return -1;
    }
    if len == 0 {
        return 0;
    }
    let end = match start.checked_add(len) { //checked_add防越界
        Some(end) => end,
        None => return -1,
    };

    // 對當前地址空间做操作。
    TASK_MANAGER.with_current_memory_set(|memory_set| {
        let start_va: VirtAddr = start.into();
        let end_va: VirtAddr = end.into();
        // 逐页解除映射，并同步裁剪/拆分对应的逻辑段
        if memory_set.unmap_range(start_va, end_va) {
            0
        } else {
            -1
        }
    })
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
