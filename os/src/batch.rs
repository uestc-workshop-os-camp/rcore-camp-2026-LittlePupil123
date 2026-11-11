//! batch subsystem
/************************************************/
// batch模块，用于使os内核程序能够获取应用程序
//对外暴露接口如下：
//  init ：调用 print_app_info 的时第一次用到了全局变量 APP_MANAGER ，
//它在这时完成初始化；
//  run_next_app ：批处理操作系统的核心操作，
//即加载并运行下一个应用程序。 批处理操作系统完成初始化，
//或者应用程序运行结束/出错后会调用该函数。下节再介绍其具体实现。
/************************************************/
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use core::arch::asm;
use lazy_static::*;

const USER_STACK_SIZE: usize = 4096 * 2;
const KERNEL_STACK_SIZE: usize = 4096 * 2;
const MAX_APP_NUM: usize = 16;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;


// 声明两个类型 KernelStack 和 UserStack 分别表示用户栈和内核栈，
// 它们都只是字节数组的简单包装：
#[repr(align(4096))]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

//--------两个栈以全局变量的形式实例化在批处理操作系统的 .bss 段中。
static KERNEL_STACK: KernelStack = KernelStack {
    data: [0; KERNEL_STACK_SIZE],
};
static USER_STACK: UserStack = UserStack {
    data: [0; USER_STACK_SIZE],
};
//-----------------------------------------------------------

//换栈是非常简单的，只需将 sp 寄存器的值修改为 对应的Stack的get_sp() 的返回值即可。
impl KernelStack {
    //实现了 get_sp 方法来获取栈顶地址。
    //由于在 RISC-V 中栈是向下增长的， 
    //我们只需返回包裹的数组的结尾地址
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    //该方法返回内核栈中trap上下文的头地址的可变指针
    //向内核栈压入初始化的上下文
    pub fn push_context(&self, cx: TrapContext) -> &'static mut TrapContext {
        let cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *cx_ptr = cx;
        }
        unsafe { cx_ptr.as_mut().unwrap() }
    }
}

impl UserStack {
    //与KernelStack同理
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

//-----------------------------------------------
//制作一个应用管理器类，

//创建结构体，内部字段声明如此
struct AppManager {
    num_app: usize, //应用程序个数
    current_app: usize, //当前需要处理的应用程序
    app_start: [usize; MAX_APP_NUM + 1], //头地址数组，内部存所有批处理中的应用程序头地址
}


//给这个结构体实现一些方法，以方便随时探查状态，和实现功能
impl AppManager {
    pub fn print_app_info(&self) { //探查AppManager目前存有的应用程序数量和每个都在哪
        println!("[kernel] num_app = {}", self.num_app);
        for i in 0..self.num_app {
            println!(
                "[kernel] app_{} [{:#x}, {:#x})",
                i,
                self.app_start[i],
                self.app_start[i + 1]
            );
        }
    }

    /// 这个方法负责将参数 app_id 对应的应用程序的二进制镜像
    /// 加载到物理内存以 0x80400000 起始的位置， 
    /// 这个位置是批处理操作系统和应用程序之间约定的常数地址。 
    /// 我们将从这里开始的一块内存清空，然后找到待加载应用二进制镜像的位置，
    /// 并将它复制到正确的位置。
    unsafe fn load_app(&self, app_id: usize) { 
        if app_id >= self.num_app {
            println!("All applications completed!");
            use crate::board::QEMUExit;
            crate::board::QEMU_EXIT_HANDLE.exit_success();
        }
        println!("[kernel] Loading app_{}", app_id);
        // clear app area
        core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);
        let app_src = core::slice::from_raw_parts(
            self.app_start[app_id] as *const u8,
            self.app_start[app_id + 1] - self.app_start[app_id],
        );
        let app_dst = core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
        app_dst.copy_from_slice(app_src);
        // Memory fence about fetching the instruction memory
        // It is guaranteed that a subsequent instruction fetch must
        // observes all previous writes to the instruction memory.
        // Therefore, fence.i must be executed after we have loaded
        // the code of the next app into the instruction memory.
        // See also: riscv non-priv spec chapter 3, 'Zifencei' extension.
        asm!("fence.i"); //这里用一条fence.i汇编指令来清理i-cache
    }

    pub fn get_current_app(&self) -> usize {
        self.current_app
    }

    pub fn move_to_next_app(&mut self) {
        self.current_app += 1;
    }
}

//创建并初始化一个全局化的AppManager实例
//这里使用了外部库 lazy_static 提供的 lazy_static! 宏。
//lazy_static! 宏提供了全局变量的运行时初始化功能。
// 一般情况下，全局变量必须在编译期设置初始值， 
// 但是有些全局变量的初始化依赖于运行期间才能得到的数据。 
// 如这里我们借助 lazy_static! 声明了一个 AppManager 结构的名为 APP_MANAGER 的全局实例， 
// 只有在它第一次被使用到的时候才会进行实际的初始化工作。
lazy_static! {
    static ref APP_MANAGER: UPSafeCell<AppManager> = unsafe {
        //通过UPSafeCell包裹整个裸结构体，防止这个全局对象被重复获取。
        UPSafeCell::new({
            extern "C" {
                fn _num_app(); //引用link_app.S里声明的_num_app符号
            }
            let num_app_ptr = _num_app as usize as *const usize; //用指针存入_num_app所指向的地址
            let num_app = num_app_ptr.read_volatile();//读取当前这个指针指向的内存地址所代表的值（即_num_app段落中的第一个元素，应用程序个数
            let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
            let app_start_raw: &[usize] =
                core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1); //将_num_app“段落”中关于存储了应用程序地址的部分在rust中转换成数组切片
            app_start[..=num_app].copy_from_slice(app_start_raw);//将得到的数组切片中的存有应用程序头地址的部分变为rust数组，并之后给结构体初始化。
            AppManager { //将从_num_app的内存段中的各个信息解析后传给AppManager实例初始化
                num_app,
                current_app: 0,
                app_start,
            }
        })
    };
}

//------------------------------------------------------

/// init batch subsystem
pub fn init() {
    print_app_info();
}

/// print apps info
pub fn print_app_info() {
    APP_MANAGER.exclusive_access().print_app_info();
}

//-------------------------------------------------------
/// run next app
pub fn run_next_app() -> ! { //run next app函数是发散类型返回值，永不返回，本质上就是上一个程序彻底坏掉，我们要完全重置出新的上下文
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.get_current_app();
    unsafe {
        app_manager.load_app(current_app); //这里完成跳转到应用程序入口点
    }
    app_manager.move_to_next_app();
    drop(app_manager);
    // 在此之前，我们必须手动 drop（丢弃）与资源相关的局部变量，
    // 并释放这些资源

    extern "C" {
        fn __restore(cx_addr: usize);
    }
    unsafe {//用__restore做从current app0到1时的上下文初始化
        __restore(KERNEL_STACK.push_context(TrapContext::app_init_context(
            APP_BASE_ADDRESS, 
            USER_STACK.get_sp(), 
        )) as *const _ as usize); //创建一个初始化的上下文到内核栈顶，然后使用restore来初始化user上下文，进入新的应用程序（或者第一个程序）
    }
    panic!("Unreachable in batch::run_current_app!");
}
