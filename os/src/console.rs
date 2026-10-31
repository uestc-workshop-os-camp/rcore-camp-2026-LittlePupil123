//! SBI console driver, for text output
use crate::sbi::console_putchar;
use core::fmt::{self, Write};

//----------------------------------------------------------
//实现基于 Write Trait 的数据结构，
//并完成 Write Trait 所需要的 write_str 函数，并用 print 函数进行包装。
struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            console_putchar(c as usize);
        }
        Ok(())
    }
}

pub fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

//----------------------------------------------------------

//实现基于 print 函数，实现Rust语言 格式化宏 ( formatting macros )
/// Print! to the host console using the format string and arguments.
#[macro_export]
macro_rules! print {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

/// Println! to the host console using the format string and arguments.
#[macro_export]
macro_rules! println {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
