//! Trap handling functionality
//!
//! For rCore, we have a single trap entry point, namely `__alltraps`. At
//! initialization in [`init()`], we set the `stvec` CSR to point to it.
//!
//! All traps go through `__alltraps`, which is defined in `trap.S`. The
//! assembly language code does just enough work restore the kernel space
//! context, ensuring that Rust code safely runs, and transfers control to
//! [`trap_handler()`].
//!
//! It then calls different functionality based on what exactly the exception
//! was. For example, timer interrupts trigger task preemption, and syscalls go
//! to [`syscall()`].

mod context;

use crate::batch::run_next_app;
use crate::syscall::syscall;
use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Trap},
    stval, stvec,
};

global_asm!(include_str!("trap.S")); //将trap.S汇编代码插入进来，保存上下文和之后的恢复

/// initialize CSR `stvec` as the entry of `__alltraps`
pub fn init() {
    extern "C" { //引入外部符号
        fn __alltraps();
    }
    unsafe { //在对系统初始化时需要修改stvec寄存器来指向正确的Trap处理入口点
        //将stvec设置为Direct模式指向__alltraps符号的地址
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
/// handle an interrupt, exception, or system call from user space
/// 在这里进行trap的分类并处理
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    //声明返回值为 &mut TrapContext 并在最后实际将传入的 cx 原样返回，
    //因此在 __restore 的时候 a0 寄存器在调用 trap_handler 前后并没有发生变化，
    //仍然指向分配 Trap 上下文之后的内核栈栈顶，和此时 sp 的值相同，我们 
    //并不会有问题；
    let scause = scause::read(); // 获取trap原因
    let stval = stval::read(); // 获取trap附加信息
    match scause.cause() { //根据scause寄存器的内容分发处理问题。这部分更底层的寄存器不需要我们操作，由Rust提供的Risc-v包解决
        Trap::Exception(Exception::UserEnvCall) => {
            //trap原因时User进行ecall主动陷入
            cx.sepc += 4; //将sepc移向ecall指令的后一条，从而保证trap处理回来后控制流能继续正常运行
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
            //调用内核态程序的模块syscall，传入需要的寄存器状态，来解决user要求的陷入
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            //处理访存错误，打印错误然后运行下一个程序
            println!("[kernel] PageFault in application, kernel killed it.");
            run_next_app();
        }
        Trap::Exception(Exception::IllegalInstruction) => {
            //处理非法指令错误，打印错误信息，然后运行下一个程序
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            run_next_app();
        }
        _ => {//对于未知错误直接崩溃退出
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx 
}
pub use context::TrapContext;
