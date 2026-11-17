//! Rust wrapper around `__switch`.
//!
//! Switching to a different task's context happens here. The actual
//! implementation must not be in Rust and (essentially) has to be in assembly
//! language (Do you know why?), so this module really is just a wrapper around
//! `switch.S`.

use super::TaskContext;
use core::arch::global_asm;

global_asm!(include_str!("switch.S"));//将这段汇编内嵌入该switch子模块

//从switch.S中引入__switch函数，并规定传入两个参数。
extern "C" {
    /// Switch to the context of `next_task_cx_ptr`, saving the current context
    /// in `current_task_cx_ptr`.
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}//我们在这里不选择直接通过符号跳转，而是包装成一个函数，
//方便编译器帮我们处理ra寄存器的状态，并帮助我们保证RISC-V格式准确
//和寄存器调用符合RISC-V标准（caller-saved和callee-saved）

