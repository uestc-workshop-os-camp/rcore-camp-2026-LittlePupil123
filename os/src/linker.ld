/*
第 1 行我们设置了目标平台为 riscv ；
第 2 行我们设置了整个程序的入口点为之前定义的全局符号 _start；
 第 3 行定义了一个常量 BASE_ADDRESS 为 0x80200000 ，
 即RustSBI 期望的 OS 起始地址；

 从 BASE_ADDRESS 开始，
 代码段 .text, 
 只读数据段 .rodata，
 数据段 .data,
 bss 段 .bss 由低到高依次放置， 
 且每个段都有两个全局变量给出其起始和结束地址
 （比如 .text 段的开始和结束地址分别是 stext 和 etext ）。
 
*/
OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;
    skernel = .;

    stext = .;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }

    . = ALIGN(4K);
    etext = .;
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    edata = .;
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}