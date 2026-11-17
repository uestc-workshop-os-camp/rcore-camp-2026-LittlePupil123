.altmacro
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch
__switch:
    # 规定__switch函数传入的两个参数，并依次将参数存入a0，a1
    # 所以a0存当前任务上下文，a1存下一个任务的上下文
    # __switch(
    #     current_task_cx_ptr: *mut TaskContext,
    #     next_task_cx_ptr: *const TaskContext
    # )

    #以a0的地址作为基准，将每个寄存器状态
    #存入我们rust程序中声明的结构体TaskContext的对应字段里。
    # save kernel stack of current task
    sd sp, 8(a0) 
    # save ra & s0~s11 of current execution
    sd ra, 0(a0)
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr

    #同理，根据a1获取到的下个任务的TaskContext结构体来恢复下一个任务上下文
    # restore ra & s0~s11 of next execution
    ld ra, 0(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr
    # restore kernel stack of next task
    ld sp, 8(a1)
    ret

