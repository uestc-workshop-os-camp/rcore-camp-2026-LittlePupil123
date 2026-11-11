.altmacro #加这个才能使用.rept来进行循环
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text
    .globl __alltraps
    .globl __restore
    .align 2    # 用 .align 将 __alltraps 的地址 4 字节对齐，这是底层硬性规定的规范
__alltraps:
    csrrw sp, sscratch, sp 
    # 将CSR当前值读到通用寄存器sp中
    # 将sp的值写入进sscratch中（实际就是交换sscratch和sp的值）
    # 原本指令格式：csrrw rd,csr,rs，
    # 信息流向：rd<--csr<--rs
    # 原本sscratch指向内核栈，sp指向用户栈，现在反过来了

    # --------------------------------------
    # 接下来的代码：保存Trap时的user上下文的通用寄存器
    # 跳过x0和tp(x4)，同时也不保存sp(x2),因为sp现在指向了内核栈，
    # 用户栈指针放在了sscratch中，要通过csrr指令读出来。
    # 因此保存其他寄存器
    # 具体每个寄存器的地址空间：
    # 栈帧分配替换后我们就可以通过sp在内核栈内部存储每个寄存器的user上下文了
    # 这些寄存器所存储的顺序时按照从低到高的形式进行的，
    # 也就是sp逐渐增加的方式存储寄存器信息


    # allocate a TrapContext on kernel stack
    addi sp, sp, -34*8
    # save general-purpose registers
    sd x1, 1*8(sp)
    # skip sp(x2), we will save it later
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    
    # save x5~x31
    .set n, 5
    .rept 27 # 做一个循环，来依次保存剩下需要被保存的寄存器(.rept)
        SAVE_GP %n # 使用SAVE_GP来保存
        .set n, n+1
    .endr
    #-------------------------
    
    # we can use t0/t1/t2 freely, because they were saved on kernel stack
    csrr t0, sstatus # 将CSR sstatus读到t0寄存器中，然后通过sp存入对应栈位置
    csrr t1, sepc # 将CSR spec读到t1寄存器中，然后通过sp存入对应栈位置
    # 这里我们不需要担心之前寄存器的内容被覆盖，因为那些寄存器内容已经通过sp存入内核栈中
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)


    # read user stack from sscratch and save it on the kernel stack
    # csrr rd,csr:将CSR的值读到寄存器rd中（rd<--CSR）
    csrr t2, sscratch # 将用户栈的栈头地址读到t2寄存器中
    sd t2, 2*8(sp) # 将t2寄存器存入内核栈中
    
    # set input argument of trap_handler(cx: &mut TrapContext)
    mv a0, sp  # 让sp的值复制给a0寄存器，这样a0就存着刚存好的user上下文栈顶
    
    call trap_handler 
    #调用trap_handler进行Trap处理，第一个参数从a0获取
    # 调用handler要用到上下文的原因是它需要知道某些寄存器的值，
    # 比如传过来的syscall ID和对应参数，但不能用当前那些寄存器的值，因为发生了改变，所以要通过a0访问原上下文（内核栈中存的）
    # 要用我们存的进入trap之前的值。


#---------------------------------------------------------------------------------------------
# 当trap_handler返回后，就开始进行恢复程序
__restore:
    # case1: start running app by __restore
    # case2: back to U after handling trap
    mv sp, a0 # 将a0的值复制到sp，也就是让sp重新指向内核栈栈顶
    # now sp->kernel stack(after allocated), sscratch->user stack
    # restore sstatus/sepc
    ld t0, 32*8(sp) # 将内核栈中存的t0状态加载到t0中
    ld t1, 33*8(sp) # 其他的同理，总之通过sp的内核栈依次访问位置依次恢复寄存器状态
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2
    # restore general-purpuse registers except sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # release TrapContext on kernel stack
    addi sp, sp, 34*8
    # now sp->kernel stack, sscratch->user stack
    csrrw sp, sscratch, sp
    sret # 在恢复上下文之后返回U状态特权级。
