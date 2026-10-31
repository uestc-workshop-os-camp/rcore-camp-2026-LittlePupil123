
    .section .text.entry # 声明或者说转移到.text.entry段，接下来汇编写在.text.entry段中
    .globl _start # 把符号 _start 声明为全局符号，使链接器和其他目标文件可见（这行没有定义这个符号）
_start: # 将符号放在这并绑定为.text.entry的开始地址，汇编器跳转到这，然后开始取指令执行（因为接下来是指令行）
    la sp, boot_stack_top # 将boot_stack_top符号的内存地址加载进sp栈指针寄存器中
    call rust_main # 调用函数，rust_main（写在main.rs中）

    
# 以下这段，我们分配了一段连续的内存地址（作为栈）    
    .section .bss.stack # 切换到.bss.stack段写汇编指令
    .globl boot_stack_lower_bound # 把boot_stack_lower_bound声明为全局
boot_stack_lower_bound: # 定义boot_stack_lower_bound符号，并绑定其所在内存地址（标记为栈底）
    .space 4096 * 16 # 分配一块4096*16字节大小的连续空间
    .globl boot_stack_top
boot_stack_top: # 定义boot_stack_top符号，其内存地址为这段空间的结束地址（栈顶）
