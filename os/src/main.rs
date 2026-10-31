//! 主模块和入口点
//!
//! 操作系统和应用程序也从这个模块启动。内核代码从 entry.asm 开始执行，
//! 之后会调用 [rust_main()] 来初始化各项功能 [clear_bss()]。（详情请参阅其源代码。）
//!
//! 然后我们调用 [println!] 来显示 Hello, world!。



#![deny(missing_docs)]
#![deny(warnings)]
#![no_std] //禁止使用std库
#![no_main] //禁用main函数，来阻止通过std提供的start语义项进行初始化
#![feature(panic_info_message)]

use core::arch::global_asm; //在main.rs中嵌入写好的汇编代码
use log::*;

#[macro_use]
mod console;     //从同名的console.rs文件中加载console模块来实现将字符输出到终端
mod lang_items; //从lang_items.rs文件加载这个模块。在这里包含了模块下的函数声明和实现
mod logging;
mod sbi;

#[path = "boards/qemu.rs"]
mod board;

global_asm!(include_str!("entry.asm")); //使用 global_asm 宏，将同目录下的汇编文件 entry.asm 嵌入到代码中。

/// clear BSS segment
pub fn clear_bss() { //清空bss段落的函数
    extern "C" { //导入链接器中的sbss和ebss全局符号，从而定位bss段落其所在的内存地址
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) }); //进行清除整个bss段落内存的操作（标记为0）
}

/// the rust entry-point of os 
#[no_mangle]  //通过宏将 rust_main 标记为 #[no_mangle] 以避免编译器对它的名字进行混淆，
//不然在链接时， entry.asm 将找不到 main.rs 提供的外部符号 rust_main，导致链接失败。
pub fn rust_main() -> ! {
    extern "C" { //导入连接器给的这些符号
        fn stext(); // begin addr of text segment
        fn etext(); // end addr of text segment
        fn srodata(); // start addr of Read-Only data segment
        fn erodata(); // end addr of Read-Only data ssegment
        fn sdata(); // start addr of data segment
        fn edata(); // end addr of data segment
        fn sbss(); // start addr of BSS segment
        fn ebss(); // end addr of BSS segment
        fn boot_stack_lower_bound(); // stack lower bound
        fn boot_stack_top(); // stack top
    }
    clear_bss();
    logging::init();
    println!("[kernel] Hello, world!");
    trace!(
        "[kernel] .text [{:#x}, {:#x})",
        stext as usize,
        etext as usize
    );
    debug!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );
    info!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );
    warn!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as usize, boot_stack_lower_bound as usize
    );
    error!("[kernel] .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);

    use crate::board::QEMUExit;
    crate::board::QEMU_EXIT_HANDLE.exit_success(); // CI autotest success
                                                   //crate::board::QEMU_EXIT_HANDLE.exit_failure(); // CI autoest failed
}
