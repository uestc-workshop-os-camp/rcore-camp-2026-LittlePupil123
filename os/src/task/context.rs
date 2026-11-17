//! Implementation of [`TaskContext`]

#[derive(Copy, Clone)]
#[repr(C)]
/// task context structure containing some registers
/// 该结构体用于存储任务上下文
pub struct TaskContext {
    /// Ret position after task switching：ra保存了switch返回后我们的控制流从哪条指令继续
    ra: usize, 
    /// Stack pointer
    sp: usize,
    /// s0-11 register, callee saved
    s: [usize; 12],
}

impl TaskContext {
    /// Create a new empty task context
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }
    /// 创建一个新的任务上下文，其中包含一个Trap返回地址和一个内核栈指针。
    /// 初始化任务上下文。
    pub fn goto_restore(kstack_ptr: usize) -> Self {
        extern "C" {
            fn __restore();
        }
        Self {
            ra: __restore as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
