//! Implementation of [`MapArea`] and [`MemorySet`].

use super::{frame_alloc, FrameTracker};
use super::{PTEFlags, PageTable, PageTableEntry};
use super::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use super::{StepByOne, VPNRange};
use crate::config::{
    KERNEL_STACK_SIZE, MEMORY_END, PAGE_SIZE, TRAMPOLINE, TRAP_CONTEXT_BASE, USER_STACK_SIZE,
};
use crate::sync::UPSafeCell;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use lazy_static::*;
use riscv::register::satp;


//引入内核中各个段落的符号
extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

lazy_static! {
    /// The kernel's initial memory mapping(kernel address space)
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<MemorySet>> =
        Arc::new(unsafe { UPSafeCell::new(MemorySet::new_kernel()) });
}
/// address space
pub struct MemorySet {
    page_table: PageTable,
    areas: Vec<MapArea>,
}


//----------------------------------------------------------
impl MemorySet {
    /// Create a new empty `MemorySet`.
    /// 新建一个空的地址空间
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }
    /// Get the page table token
    pub fn token(&self) -> usize {
        self.page_table.token()
    }
    /// Assume that no conflicts.
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }
    /// Check whether `[start_va, end_va)` overlaps none of the existing areas.
    ///
    /// Used by `mmap` to refuse a mapping that would clobber an existing one.
    pub fn is_range_free(&self, start_va: VirtAddr, end_va: VirtAddr) -> bool {
        let start = start_va.floor();
        let end = end_va.ceil();
        self.areas
            .iter()
            .all(|area| area.vpn_range.get_end() <= start || end <= area.vpn_range.get_start())
    }
    /// Unmap every page of `[start_va, end_va)`.
    pub fn unmap_range(&mut self, start_va: VirtAddr, end_va: VirtAddr) -> bool {
        //获取区间的起始页和终止页
        let start_vpn = start_va.floor();
        let end_vpn = end_va.ceil();
        if start_vpn >= end_vpn {
            return false;
        }
        // 先确认区间内每一vpn都建立了映射，并且区间内有落在地址空间已有的某一个逻辑段上
        let mut vpn = start_vpn;
        while vpn < end_vpn {
            let in_area = self
                .areas
                .iter()
                //确定vpn是否在某个逻辑段，以及该vpn是否被映射
                .any(|area| area.vpn_range.get_start() <= vpn && vpn < area.vpn_range.get_end());
            if !in_area || self.page_table.translate(vpn).is_none() {//必须保证区间内存在vpn都在地址空间现有逻辑段以及都有映射才开始解除映射
                return false;
            }
            vpn.step();//虚拟区间内的下一个虚拟页号
        }
        //分别取出当前地址空间的逻辑段集以及页表这两个可变引用方便后续使用
        let MemorySet { areas, page_table } = self;
        let mut vpn = start_vpn;
        while vpn < end_vpn { //迭代给定的页区间
            // 找到包含当前页（实际是起始页）的逻辑段
            let idx = match areas.iter().position(|area| {
                area.vpn_range.get_start() <= vpn && vpn < area.vpn_range.get_end()
            }) {
                Some(idx) => idx,
                None => {
                    vpn.step();
                    continue;
                }
            };
            //获取这个逻辑段的起始页和终止页
            let area_start = areas[idx].vpn_range.get_start();
            let area_end = areas[idx].vpn_range.get_end();
            // 确定找到的逻辑段区间和给定区间相交集合（在当前逻辑段内能够删除的区间）
            let unmap_start = area_start.max(start_vpn);
            let unmap_end = area_end.min(end_vpn);

            // 逐页解除映射：释放物理页帧并清空页表项
            let mut page = unmap_start;
            while page < unmap_end {
                if page_table.translate(page).is_some() {
                    areas[idx].unmap_one(page_table, page);
                }
                page.step();
            }

            // 根据交集在逻辑段中的位置修改它的 vpn_range
            if unmap_start == area_start && unmap_end == area_end {
                // 整个逻辑段都被解除，直接删掉
                areas.remove(idx);
            } else if unmap_start == area_start {
                // 只解除头部，段起点右移
                areas[idx].vpn_range = VPNRange::new(unmap_end, area_end);
            } else if unmap_end == area_end {
                // 只解除尾部，段终点左移
                areas[idx].vpn_range = VPNRange::new(area_start, unmap_start);
            } else {
                // 落在逻辑段中间：拆成两段
                
                let right_frames = areas[idx].data_frames.split_off(&unmap_end);//提出右边的段落

                //对剩下的左边逻辑段修改边界，保存其他属性，方便后续新建右段落
                let map_type = areas[idx].map_type;
                let map_perm = areas[idx].map_perm;
                areas[idx].vpn_range = VPNRange::new(area_start, unmap_start);

                //插入新生成的右边逻辑段进入areas数组管理，除了VPNRange之外的属性和左边段落一致
                areas.insert(
                    idx + 1,
                    MapArea {
                        vpn_range: VPNRange::new(unmap_end, area_end),
                        data_frames: right_frames,
                        map_type,
                        map_perm,
                    },
                );
            }
            vpn = unmap_end;//vpn右移，缩小未修改的vpn区间
        }
        true
    }
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&mut self.page_table, data);
        }
        self.areas.push(map_area);
    }
    /// Mention that trampoline is not collected by areas.
    fn map_trampoline(&mut self) {
        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }
    /// Without kernel stacks.
    /// 创建内核地址空间
    pub fn new_kernel() -> Self {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map kernel sections
        info!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        info!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        info!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        info!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );
        info!("mapping .text section");
        memory_set.push(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        info!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        info!("mapping .data section");
        memory_set.push(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        info!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        memory_set
    }
    /// Include sections in elf and trampoline and TrapContext and user stack,
    /// also returns user_sp_base and entry point.
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        // map trampoline
        memory_set.map_trampoline();
        // map program headers of elf, with U flag
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        // map user stack with U flags
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_bottom: usize = max_end_va.into();
        // guard page
        user_stack_bottom += PAGE_SIZE;
        let user_stack_top = user_stack_bottom + USER_STACK_SIZE;
        memory_set.push(
            MapArea::new(
                user_stack_bottom.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // used in sbrk
        memory_set.push(
            MapArea::new(
                user_stack_top.into(),
                user_stack_top.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W | MapPermission::U,
            ),
            None,
        );
        // map TrapContext
        memory_set.push(
            MapArea::new(
                TRAP_CONTEXT_BASE.into(),
                TRAMPOLINE.into(),
                MapType::Framed,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        (
            memory_set,
            user_stack_top,
            elf.header.pt2.entry_point() as usize,
        )
    }
    /// Change page table by writing satp CSR Register.
    pub fn activate(&self) {
        let satp = self.page_table.token();
        unsafe {
            satp::write(satp);
            asm!("sfence.vma");
        }
    }
    /// Translate a virtual page number to a page table entry
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }
    /// shrink the area to new_end
    #[allow(unused)]
    pub fn shrink_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.shrink_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }

    /// append the area to new_end
    #[allow(unused)]
    pub fn append_to(&mut self, start: VirtAddr, new_end: VirtAddr) -> bool {
        if let Some(area) = self
            .areas
            .iter_mut()
            .find(|area| area.vpn_range.get_start() == start.floor())
        {
            area.append_to(&mut self.page_table, new_end.ceil());
            true
        } else {
            false
        }
    }
}
///----------------------------------------------------------

//MapArea相关声明和实现
/// map area structure, controls a contiguous piece of virtual memory
pub struct MapArea {
    vpn_range: VPNRange,
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    map_type: MapType,
    map_perm: MapPermission,
}

//----------------------------------------------------------
impl MapArea {
    ///通过传入签名新建一个逻辑段
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }

    /// 在虚拟页号 vpn 已经确定的情况下，
    /// 它需要知道要将一个怎么样的页表项插入多级页表。
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {//根据不同的映射类型来为该页表项分配物理页帧
            MapType::Identical => {//恒等映射类型，直接将vpn转换为对应相等的ppn
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {//Framed映射类型，为当前页表项分配一个页帧
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);//对应物理页帧插入至data_frames中，方便统一管理数据段的页帧
            }
        }
        //而标志位则统一通过逻辑段给的map_perm来设置转换。
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);//然后拼接并形成一个完整的页表项插入进页表中
        //只有成功插入进页表，对应的地址空间下的逻辑段才有了对应的实际物理页帧能够操作
    }

    /// 解除逻辑段中某一页的映射关系。
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {//data_frames只在Framed类型时才不为空
            self.data_frames.remove(&vpn);//释放对应的物理页帧
        }
        page_table.unmap(vpn);//页表中删除对应的页表项
    }
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }
    #[allow(unused)]
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }
    #[allow(unused)]
    pub fn shrink_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(new_end, self.vpn_range.get_end()) {
            self.unmap_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    #[allow(unused)]
    pub fn append_to(&mut self, page_table: &mut PageTable, new_end: VirtPageNum) {
        for vpn in VPNRange::new(self.vpn_range.get_end(), new_end) {
            self.map_one(page_table, vpn)
        }
        self.vpn_range = VPNRange::new(self.vpn_range.get_start(), new_end);
    }
    /// data: start-aligned but maybe with shorter length
    /// assume that all frames were cleared before
    /// 将data拷贝到逻辑段对应的一系列物理页帧中
    /// 切片 data 中的数据大小不超过当前逻辑段的总大小，
    /// 且切片中的数据会被对齐到逻辑段的开头，然后逐页拷贝到实际的物理页帧。
    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {//loop内进行逐页拷贝，遍历每一个需要拷贝数据的虚拟页面
            //确定复制源src和目的dst以及每页能复制的原data的长度
            let src = &data[start..len.min(start + PAGE_SIZE)];//取长度和最大页长间的最小值
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len { //进入下一页前判断自己是否已经复制完成
                break;
            }
            current_vpn.step();
        }
    }
}
//----------------------------------------------------------


#[derive(Copy, Clone, PartialEq, Debug)]
/// map type for memory set: identical or framed
pub enum MapType {
    Identical,
    Framed,
}

bitflags! {
    /// map permission corresponding to that in pte: `R W X U`
    pub struct MapPermission: u8 {
        ///Readable
        const R = 1 << 1;
        ///Writable
        const W = 1 << 2;
        ///Excutable
        const X = 1 << 3;
        ///Accessible in U mode
        const U = 1 << 4;
    }
}

/// Return (bottom, top) of a kernel stack in kernel space.
pub fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

/// remap test in kernel space
#[allow(unused)]
pub fn remap_test() {
    let mut kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert!(!kernel_space
        .page_table
        .translate(mid_text.floor())
        .unwrap()
        .writable(),);
    assert!(!kernel_space
        .page_table
        .translate(mid_rodata.floor())
        .unwrap()
        .writable(),);
    assert!(!kernel_space
        .page_table
        .translate(mid_data.floor())
        .unwrap()
        .executable(),);
    println!("remap_test passed!");
}
