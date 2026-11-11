//! App management syscalls
use crate::batch::run_next_app;

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {//打印应用程序的退出码然后进入下一个程序
    trace!("[kernel] Application exited with code {}", exit_code);
    run_next_app()
}
