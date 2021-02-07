#![allow(clippy::identity_op)]

use crate::utils;
use cortex_a::barrier::*;
use register::register_bitfields;

use core::fmt::Write;

use crate::m1::uart::UART;

extern "C" {
    static mut __text_start__: u8;
    static mut __text_end__: u8;
    static mut __vectors_start__: u8;
    static mut __vectors_end__: u8;
    static mut __rodata_start__: u8;
    static mut __rodata_end__: u8;
    static mut __data_start__: u8;
    static mut __data_end__: u8;
    static mut __bss_start__: u8;
    static mut __bss_end__: u8;
    static _stack_bottom: u8;
    static _stack_top: u8;
}

const PAGE_GRANULE_4K: usize = 12;
const PAGE_GRANULE_16K: usize = 14;
const PAGE_GRANULE_64K: usize = 16;

const PAGE_GRANULE: usize = PAGE_GRANULE_16K;

const ENTRY_SHIFT: usize = 3;
const ENTRIES_PER_LEVEL_BITS: usize = PAGE_GRANULE - ENTRY_SHIFT;
const ENTRIES_PER_LEVEL: usize = 1 << ENTRIES_PER_LEVEL_BITS;

const L3_INDEX_LSB: usize = PAGE_GRANULE;
const L2_INDEX_LSB: usize = L3_INDEX_LSB + ENTRIES_PER_LEVEL_BITS;
const L1_INDEX_LSB: usize = L2_INDEX_LSB + ENTRIES_PER_LEVEL_BITS;
const L0_INDEX_LSB: usize = L1_INDEX_LSB + ENTRIES_PER_LEVEL_BITS;

// 48 bits address space
const TARGET_BITS: usize = 48;

const NUM_LVL1_ENTRIES: usize = 0x4;

const TCR_IPS_1TB: u64 = ((0b010) << 32);
const TCR_TG1_16K: u64 = ((0b01) << 30);
const TCR_SH1_IS: u64 = ((0b11) << 28);
const TCR_ORGN1_WBWA: u64 = ((0b01) << 26);
const TCR_IRGN1_WBWA: u64 = ((0b01) << 24);
const TCR_T1SZ_48BIT: u64 = ((0b101) << 16);
const TCR_TG0_16K: u64 = ((0b10) << 14);
const TCR_SH0_IS: u64 = ((0b11) << 12);
const TCR_ORGN0_WBWA: u64 = ((0b01) << 10);
const TCR_IRGN0_WBWA: u64 = ((0b01) << 8);
const TCR_T0SZ_48BIT: u64 = ((16) << 0);

#[repr(C)]
#[repr(align(0x4000))]
#[derive(Copy, Clone)]
struct TableLVL0 {
    entries: [u64; 2],
    lvl1: [TableLVL1; 2],
}

#[repr(C)]
#[repr(align(0x4000))]
#[derive(Copy, Clone)]
struct TableLVL2 {
    entries: [u64; ENTRIES_PER_LEVEL],
}

#[repr(C)]
#[repr(align(0x4000))]
#[derive(Copy, Clone)]
struct TableLVL1 {
    entries: [u64; ENTRIES_PER_LEVEL],
    lvl2: [TableLVL2; NUM_LVL1_ENTRIES],
}

static mut LVL0_TABLE: TableLVL0 = TableLVL0 {
    entries: [0x0; 2],
    lvl1: [TableLVL1 {
        entries: [0x0; ENTRIES_PER_LEVEL],
        lvl2: [TableLVL2 {
            entries: [0x0; ENTRIES_PER_LEVEL],
        }; NUM_LVL1_ENTRIES],
    }; 2],
};

static mut next_lvl1_index: usize = 0;

#[derive(Copy, Clone, PartialEq)]
pub enum MemoryPermission {
    Invalid,
    R,
    W,
    X,
    RW,
    RX,
    RWX,
}

register_bitfields! {u64,
    STAGE1_NEXTLEVEL_DESCRIPTOR [
        VALID OFFSET(0) NUMBITS(1) [
            True = 1
        ],

        TYPE OFFSET(1) NUMBITS(1) [
            Table = 1
        ],

        ADDRESS_4K OFFSET(12) NUMBITS(36) [],
        ADDRESS_16K OFFSET(14) NUMBITS(34) [],
        ADDRESS_64K OFFSET(16) NUMBITS(32) [],

        PXN OFFSET(59) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        XN OFFSET(60) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        AP_TABLE OFFSET(61) NUMBITS(2) [
            NO_EFFECT = 0b00,
            NO_EL0 = 0b01,
            NO_WRITE = 0b10,
            NO_WRITE_EL0_READ = 0b11
        ],

        NS OFFSET(63) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

register_bitfields! {u64,
    STAGE2_BLOCK_DESCRIPTOR [
        VALID OFFSET(0) NUMBITS(1) [
            True = 1
        ],

        TYPE OFFSET(1) NUMBITS(1) [
            Block = 0
        ],

        MEMORY_ATTR OFFSET(2) NUMBITS(4) [],

        AP OFFSET(6) NUMBITS(2) [
            RW_CURRENT_EL = 0b00,
            RW_BOTH_EL = 0b01,
            RO_CURRENT_EL = 0b10,
            RO_BOTH_EL = 0b11
        ],

        SH OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        AF OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        ADDRESS_4K OFFSET(21) NUMBITS(27) [],
        ADDRESS_16K OFFSET(25) NUMBITS(23) [],
        ADDRESS_64K OFFSET(29) NUMBITS(19) [],

        CONTIGUOUS OFFSET(52) NUMBITS(1) [],

        XN OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]

}

register_bitfields! {u64,
    STAGE2_NEXTLEVEL_DESCRIPTOR [
        VALID OFFSET(0) NUMBITS(1) [
            True = 1
        ],

        TYPE OFFSET(1) NUMBITS(1) [
            Table = 1
        ],

        ADDRESS_4K OFFSET(12) NUMBITS(36) [],
        ADDRESS_16K OFFSET(14) NUMBITS(34) [],
        ADDRESS_64K OFFSET(16) NUMBITS(32) []
    ]
}

register_bitfields! {u64,
    STAGE3_TABLE_DESCRIPTOR [
        VALID OFFSET(0) NUMBITS(1) [
            True = 1
        ],

        TYPE OFFSET(1) NUMBITS(1) [
            Table = 1
        ],

        MEMORY_ATTR OFFSET(2) NUMBITS(4) [],

        AP OFFSET(6) NUMBITS(2) [
            RW_CURRENT_EL = 0b00,
            RW_BOTH_EL = 0b01,
            RO_CURRENT_EL = 0b10,
            RO_BOTH_EL = 0b11
        ],

        SH OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],

        AF OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],

        ADDRESS OFFSET(12) NUMBITS(36) [],

        XN OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

pub mod mem_attr {
    // Normal memory
    pub const NORMAL: u64 = 0;

    // Device-nGnRnE
    pub const MMIO_nGnRnE: u64 = 1;

    // Device-nGnRE
    pub const MMIO_nGnRE: u64 = 2;
}

unsafe fn get_lvl2_table(vaddr: u64) -> Option<&'static mut TableLVL2> {
    let lvl1_align_size = 1 << L1_INDEX_LSB;

    let lvl1_index = (vaddr / lvl1_align_size) as usize % ENTRIES_PER_LEVEL;

    if next_lvl1_index == NUM_LVL1_ENTRIES {
        return None;
    }

    let mut lvl1_entry = LVL0_TABLE.lvl1[0].entries[lvl1_index];

    if lvl1_entry == 0 {
        writeln!(
            &mut UART::INSTANCE,
            "Creating new lvl2 table 0x{:x} (0x{:x})",
            lvl1_index,
            vaddr
        )
        .ok();

        create_lvl1_table_entry(
            vaddr,
            &LVL0_TABLE.lvl1[0].lvl2[next_lvl1_index].entries[0] as *const _ as u64,
        );

        next_lvl1_index += 1;

        lvl1_entry = LVL0_TABLE.lvl1[0].entries[lvl1_index];
    }

    lvl1_entry &= (1 << L0_INDEX_LSB) - 1;
    lvl1_entry &= !((1 << PAGE_GRANULE) - 1);

    (lvl1_entry as *mut TableLVL2).as_mut()
}

fn create_lvl2_block_entry(vaddr: u64, paddr: u64, memory_attribute: u64) {
    let lvl2_align_size = 1 << L2_INDEX_LSB;

    let lvl2_index = (vaddr / lvl2_align_size) as usize % ENTRIES_PER_LEVEL;

    let flags = STAGE2_BLOCK_DESCRIPTOR::VALID::True
        + STAGE2_BLOCK_DESCRIPTOR::TYPE::Block
        + STAGE2_BLOCK_DESCRIPTOR::MEMORY_ATTR.val(memory_attribute)
        + STAGE2_BLOCK_DESCRIPTOR::AF::True
        + STAGE2_BLOCK_DESCRIPTOR::SH::InnerShareable;

    unsafe {
        let table = get_lvl2_table(vaddr).expect("Out of LVL2 tables");

        table.entries[lvl2_index] =
            (flags + STAGE2_BLOCK_DESCRIPTOR::ADDRESS_16K.val(paddr >> L2_INDEX_LSB)).value;

        dsb(SY);
    };
}

fn create_lvl1_table_entry(vaddr: u64, table_address: u64) {
    let lvl1_align_size = 1 << L1_INDEX_LSB;

    let lvl1_index = (vaddr / lvl1_align_size) as usize % ENTRIES_PER_LEVEL;

    unsafe {
        LVL0_TABLE.lvl1[0].entries[lvl1_index] = (STAGE1_NEXTLEVEL_DESCRIPTOR::VALID::True
            + STAGE1_NEXTLEVEL_DESCRIPTOR::TYPE::Table
            + STAGE1_NEXTLEVEL_DESCRIPTOR::ADDRESS_16K.val(table_address >> PAGE_GRANULE))
        .value;

        dsb(SY);
    };
}

fn map_lvl2_block(vaddr: u64, paddr: u64, size: u64, memory_attribute: u64) {
    let lvl2_align_size = 1 << L2_INDEX_LSB;
    let size = utils::align_up(size, lvl2_align_size);

    let mut vaddr = utils::align_down(vaddr, lvl2_align_size);
    let mut paddr = utils::align_down(paddr, lvl2_align_size);
    let mut page_count = size / lvl2_align_size;

    while page_count != 0 {
        create_lvl2_block_entry(vaddr, paddr, memory_attribute);
        vaddr += lvl2_align_size;
        paddr += lvl2_align_size;
        page_count -= 1;
    }
}

fn init_page_mapping() {
    // Setup LVL0 entries
    unsafe {
        for (lvl0_index, lvl0_entry) in LVL0_TABLE.entries.iter_mut().enumerate() {
            let table_address = &LVL0_TABLE.lvl1[lvl0_index].entries[0] as *const _ as u64;

            *lvl0_entry = (STAGE1_NEXTLEVEL_DESCRIPTOR::VALID::True
                + STAGE1_NEXTLEVEL_DESCRIPTOR::TYPE::Table
                + STAGE1_NEXTLEVEL_DESCRIPTOR::ADDRESS_16K.val(table_address >> PAGE_GRANULE))
            .value;
        }
    }

    // map MMIOs
    const MMIO_RANGE_0_ADDR: u64 = 0x0000000000;
    const MMIO_RANGE_0_SIZE: u64 = 0x0800000000;

    // Map the known MMIO range as nGnRnE with an identity mapping.
    map_lvl2_block(
        MMIO_RANGE_0_ADDR,
        MMIO_RANGE_0_ADDR,
        MMIO_RANGE_0_SIZE,
        mem_attr::MMIO_nGnRnE,
    );

    // Map the known MMIO range as nGnRE at 0xF000000000
    map_lvl2_block(
        0xf000000000,
        MMIO_RANGE_0_ADDR,
        MMIO_RANGE_0_SIZE,
        mem_attr::MMIO_nGnRE,
    );

    // Map 16GB of normal memory
    map_lvl2_block(0x0800000000, 0x0800000000, 0x0400000000, mem_attr::NORMAL);
}

fn get_sctlr() -> u64 {
    let mut ctrl: u64;

    unsafe {
        match utils::get_current_el() {
            1 => asm!("mrs {sctlr}, sctlr_el1", sctlr = out(reg) ctrl, options(nostack)),
            2 => asm!("mrs {sctlr}, sctlr_el2", sctlr = out(reg) ctrl, options(nostack)),
            _ => unimplemented!(),
        }
    }

    ctrl
}

fn set_sctlr(new_sctlr: u64) {
    unsafe {
        match utils::get_current_el() {
            1 => asm!("msr sctlr_el1, {sctlr}", sctlr = in(reg) new_sctlr, options(nostack)),
            2 => asm!("msr sctlr_el2, {sctlr}", sctlr = in(reg) new_sctlr, options(nostack)),
            _ => unimplemented!(),
        }

        isb(SY);
    }
}

pub fn invalidate_tlb_all() {
    unsafe {
        match utils::get_current_el() {
            1 => asm!("tlbi vmalle1"),
            2 => asm!("tlbi alle2"),
            _ => unimplemented!(),
        }

        dsb(SY);
        isb(SY);
    }
}

pub fn invalidate_icache_all() {
    unsafe {
        asm!("ic iallu");
        dsb(SY);
        isb(SY);
    }
}

unsafe fn set_mair_ttbr_tcr(mair: u64, ttbr: u64, tcr: u64) {
    match utils::get_current_el() {
        1 => {
            asm!(
                "
                msr mair_el1, {mair}
                msr tcr_el1, {tcr}
                msr ttbr0_el1, {ttbr}
                msr ttbr1_el1, {ttbr}
                ",
                mair = in(reg) mair,
                ttbr = in(reg) ttbr,
                tcr = in(reg) tcr,
                options(nostack),
            );
        }
        2 => {
            asm!(
                "
                msr mair_el2, {mair}
                msr tcr_el2, {tcr}
                msr ttbr0_el2, {ttbr}
                msr ttbr1_el2, {ttbr}
                ",
                mair = in(reg) mair,
                ttbr = in(reg) ttbr,
                tcr = in(reg) tcr,
                options(nostack),
            );
        }
        _ => unimplemented!(),
    }

    dsb(ISHST);
    asm!("tlbi vmalls12e1is");
    dsb(ISH);
    isb(SY);
}

pub unsafe fn setup() {
    let mut uart = &mut UART::INSTANCE;

    writeln!(&mut uart, "Configuring MMU...").ok();
    init_page_mapping();

    // Setup memory attributes, lvl1 table and tcr.
    let mair: u64 = (0x00 << (mem_attr::MMIO_nGnRnE * 8))
        | (0x00 << (mem_attr::MMIO_nGnRE * 8))
        | (0xFF << (mem_attr::NORMAL * 8));

    let ttbr = &LVL0_TABLE.entries[0] as *const _ as u64;

    // Taken from m1n1
    // TODO: improve this
    let tcr: u64 = TCR_IPS_1TB
        | TCR_TG1_16K
        | TCR_SH1_IS
        | TCR_ORGN1_WBWA
        | TCR_IRGN1_WBWA
        | TCR_T1SZ_48BIT
        | TCR_TG0_16K
        | TCR_SH0_IS
        | TCR_ORGN0_WBWA
        | TCR_IRGN0_WBWA
        | ((64 - TARGET_BITS as u64) << 0);
    set_mair_ttbr_tcr(mair, ttbr, tcr);

    // Invalidate icache as we are going to activate it.
    invalidate_icache_all();

    // finally enable MMU and cache
    let mut ctrl = get_sctlr();

    ctrl |= 0xC00800; // mandatory reserved bits
    ctrl |= (1 << 12) |    // I, Instruction cache enable. This is an enable bit for instruction caches at EL0 and EL1
            (1 << 4)  |    // SA0, Stack Alignment Check Enable for EL0
            (1 << 3)  |    // SA, Stack Alignment Check Enable
            (1 << 2)  |    // C, Data cache enable. This is an enable bit for data caches at EL0 and EL1
            (1 << 1)  |    // A, Alignment check enable bit
            (1 << 0); // set M, enable MMU

    set_sctlr(ctrl);

    // and hope that it's okayish
    invalidate_tlb_all();

    writeln!(&mut uart, "MMU configured").ok();
}
