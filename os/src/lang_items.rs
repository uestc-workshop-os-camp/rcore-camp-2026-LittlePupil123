//! The panic handler

use crate::sbi::shutdown;
use core::panic::PanicInfo;

#[panic_handler] //告知编译器采用这套实现来取代原本std库中的panic_handler
/// panic handler
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "[kernel] Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        println!("[kernel] Panicked: {}", info.message().unwrap());
    }
    shutdown()
}
