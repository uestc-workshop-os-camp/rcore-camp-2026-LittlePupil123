//! File and filesystem-related syscalls

const FD_STDOUT: usize = 1;

/// write buf of length `len`  to a file with `fd`
/// 这里我们会将传进来的参数 args 转化成能够被具体的系统调用处理函数接受的类型。
/// 它们的实现都非常简单
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel: sys_write");
    match fd {
        FD_STDOUT => {
            let slice = unsafe { core::slice::from_raw_parts(buf, len) };
            let str = core::str::from_utf8(slice).unwrap();
            print!("{}", str);
            len as isize
        }
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}
