1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

《你交流的对象说明》
ch1，2的os目录下的batch子模块，task子模块，entry.asm，trap子模块，syscall子模块，loader子模块等；user目录下的link.S以及lang_items.rs，lib库包

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

《你参考的资料说明》
实验文档和rCore Tutorial Book v3


3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

# 实现功能
完成了sys_trace()系统调用接口的实现，能够读取id符号处地址的值，写入值，以及通过新声明与初始化在TCB结构体下的syscall_times[]字段实现获取每个应用程序调用对应系统调用接口的次数。

# 问答题
1.
对应报错如下：
```
OpenSBI v1.3

[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
```
第一个bad程序出错行为：在U态强制读写一个未被规定的物理内存地址。预期触发trap_handler下的storeFault分支。traphandler从scause寄存器中识别该错误，打印该输出，并中断当前程序执行下一个程序。
第二个bad程序出错行为：在U态调用sret，非U态指令。预期触发InllegalInstruction分支。traphandler识别该错误并打印该输出并退出当前程序执行下一个程序
第三个程序出错行为：在U态试图修改S态特权寄存器，预期触发InllegailInstruction分支。。。

2.
（1）内核栈地址。两种情形：程序第一次进入用户态；程序从S态恢复成原U态，并恢复其上下文
（2）恢复了原任务的sstatus，sscratch，sepc寄存器。sstatus恢复为trap发生之前的特权级（恢复为U）；sscratch负责读取当前任务的用户栈，方便后续与sp交换从而使sp进入用户栈，sscratch继续保管当前任务的内核栈；sepc存储着trap前的最后一条指令地址，方便程序sret后跳转到sepc指向的位置继续执行该应用程序

3.x2是sp寄存器，在当前的指令下指向内核栈顶的trapContext上下文，如果这时候恢复就会出现sp不再指向内核栈的trap上下文保存栈帧处，导致无法进行后续恢复，sp的恢复是最后和sscratch交换指向用户栈。x4是tp寄存器，用不到。

4.交换后sp指向用户栈，sscratch存储当前任务的内核栈

5.sret指令。硬件底层规定，且对应的用户上下文也均恢复成trap前的状态（包括sepc等特权寄存器），sp也指向了用户栈。

6.sp和sscratch互换，sp指向该任务的内核栈顶地址，sscratch暂存用户栈顶地址

7.调用ecall，硬件处理一部分寄存器，然后跳转到stvec指向的__alltraps符号