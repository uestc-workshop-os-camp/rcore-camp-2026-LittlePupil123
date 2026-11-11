use riscv::register::sstatus::{self, Sstatus, SPP};
/// Trap Context
/// Trap 上下文，即在 Trap 发生时需要保存的物理资源内容
#[repr(C)]
pub struct TrapContext {
    /// general regs[0..31]
    /// 里面包含所有的通用寄存器 x0~x31 ，
    /// 还有 sstatus 和 sepc 。
    pub x: [usize; 32],
    /// CSR sstatus      
    pub sstatus: Sstatus,
    /// CSR sepc
    pub sepc: usize,
}

impl TrapContext {
    /// set stack pointer to x_2 reg (sp)
    pub fn set_sp(&mut self, sp: usize) { //将trapcontext的x2寄存器初始给定sp的值（sp寄存器初始指向用户栈）
        self.x[2] = sp;
    }
    /// init app context
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        let mut sstatus = sstatus::read(); // CSR sstatus
        sstatus.set_spp(SPP::User); //刷新sstatus寄存器状态为U态
        let mut cx = Self { //各寄存器状态初始化
            x: [0; 32],
            sstatus,
            sepc: entry, // 将sepc寄存器地址指向规定的应用程序入口地址处
        };
        cx.set_sp(sp); // 给cx的上下文恢复为sp寄存器所存的地址
        cx // 返回用户态上下文
    }
}
