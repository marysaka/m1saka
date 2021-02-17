//! Based on m1n1 memory setup (Copyright (c) 2021 The Asahi Linux contributors) 
//! https://github.com/AsahiLinux/m1n1/blob/main/src/memory.c
//! 
//! TODO: rewrite and extends this to our needs (possibly by not sharing similarities to m1n1's mmu setup).
#![allow(clippy::identity_op)]

use crate::utils;
use cortex_a::barrier::*;
use register::register_bitfields;

use core::fmt::Write;

use crate::m1::uart::UART;

use alloc::alloc::Layout;

const PAGE_GRANULE: usize = 14;

const ENTRY_SHIFT: usize = 3;
const ENTRIES_PER_LEVEL_BITS: usize = PAGE_GRANULE - ENTRY_SHIFT;
const ENTRIES_PER_LEVEL: usize = 1 << ENTRIES_PER_LEVEL_BITS;

const L3_PAGE_SIZE: u64 = 1 << PAGE_GRANULE as u64;
const L2_PAGE_SIZE: u64 = 1 << (PAGE_GRANULE + ENTRIES_PER_LEVEL_BITS) as u64;
const L1_PAGE_SIZE: u64 = 1 << (PAGE_GRANULE + ENTRIES_PER_LEVEL_BITS + ENTRIES_PER_LEVEL_BITS) as u64;
const L0_PAGE_SIZE: u64 = 1 << (PAGE_GRANULE + ENTRIES_PER_LEVEL_BITS + ENTRIES_PER_LEVEL_BITS + ENTRIES_PER_LEVEL_BITS) as u64;

const PTE_TYPE_BLOCK: u64 = 0b01;
const PTE_TYPE_TABLE: u64 = 0b11;
const PTE_FLAG_ACCESS: u64 = 1 << 10;

const PTE_AP_RO: u64 = 1 << 7;
const PTE_PXN: u64 = 1 << 53;
const PTE_UXN: u64 = 1 << 54;

const PERMISSION_RO: u64 = PTE_AP_RO | PTE_PXN | PTE_UXN;
const PERMISSION_RW: u64 = PTE_PXN | PTE_UXN;
const PERMISSION_RWX: u64 = 0;

const TCR_IPS_1TB: u64 = 0b010 << 32;
const TCR_TG1_16K: u64 = 0b01 << 30;
const TCR_SH1_IS: u64 = 0b11 << 28;
const TCR_ORGN1_WBWA: u64 = 0b01 << 26;
const TCR_IRGN1_WBWA: u64 = 0b01 << 24;
const TCR_T1SZ_48BIT: u64 = 0b101 << 16;
const TCR_TG0_16K: u64 = 0b10 << 14;
const TCR_SH0_IS: u64 = 0b11 << 12;
const TCR_ORGN0_WBWA: u64 = 0b01 << 10;
const TCR_IRGN0_WBWA: u64 = 0b01 << 8;
const TCR_T0SZ_48BIT: u64 = 16 << 0;

pub mod mem_attr {
    // Normal memory
    pub const NORMAL: u8 = 0;

    // Device-nGnRnE
    pub const DEVICE_nGnRnE: u8 = 1;

    // Device-nGnRE
    pub const DEVICE_nGnRE: u8 = 2;
}

#[repr(C)]
#[repr(align(0x4000))]
struct TableLVL0 {
    entries: [u64; 2],
}

#[repr(C)]
#[repr(align(0x4000))]
struct LevelTable {
    entries: [u64; ENTRIES_PER_LEVEL],
}

static mut LVL0_TABLE: TableLVL0 = TableLVL0 {
    entries: [0x0 ; 2]
};

static mut LVL1_TABLE: [LevelTable; 2] = [LevelTable {
    entries: [0x0 ; ENTRIES_PER_LEVEL]
}, LevelTable {
    entries: [0x0 ; ENTRIES_PER_LEVEL]
}];

unsafe fn create_block_page_table_entry(addr: u64, attr: u8, permissions: u64) -> u64 {
    PTE_TYPE_BLOCK | addr | PTE_FLAG_ACCESS | (u64::from(attr) & 7) << 2 | permissions
}

unsafe fn create_table_page_table_entry(addr: u64) -> u64 {
    PTE_TYPE_TABLE | addr | PTE_FLAG_ACCESS
}

unsafe fn get_lvl2_table(virtual_address: u64) -> Option<&'static mut LevelTable> {
    let lvl0_index = (virtual_address / L0_PAGE_SIZE) as usize % 2;
    let lvl1_index = (virtual_address / L1_PAGE_SIZE) as usize % ENTRIES_PER_LEVEL;

    let mut lvl1_entry = LVL1_TABLE[lvl0_index].entries[lvl1_index];

    if lvl1_entry == 0 {
        writeln!(
            &mut UART::INSTANCE,
            "Creating new lvl2 table 0x{:x} (0x{:x})",
            lvl1_index,
            virtual_address
        )
        .ok();

        let lvl2_table_layout = Layout::new::<LevelTable>();
        let lvl2_table = alloc::alloc::alloc(lvl2_table_layout);

        writeln!(
            &mut UART::INSTANCE,
            "{:p}",
            lvl2_table,
        )
        .ok();

        LVL1_TABLE[lvl0_index].entries[lvl1_index] = create_table_page_table_entry(lvl2_table as u64);

        lvl1_entry = LVL1_TABLE[lvl0_index].entries[lvl1_index];
    }

    lvl1_entry &= (1 << 48) - 1;
    lvl1_entry &= !((1 << PAGE_GRANULE) - 1);

    (lvl1_entry as *mut LevelTable).as_mut()
}

unsafe fn add_single_lvl2_mapping(virtual_address: u64, physical_address: u64,  attribute: u8, permissions: u64) {
    let table_lvl2 = get_lvl2_table(virtual_address).expect("Cannot allocate lvl2 table!");

    let lvl2_index = (virtual_address / L2_PAGE_SIZE) as usize % ENTRIES_PER_LEVEL;

    assert!(table_lvl2.entries[lvl2_index] == 0, "lvl2 entry already allocated!");

    table_lvl2.entries[lvl2_index] = create_block_page_table_entry(physical_address, attribute, permissions);

    //writeln!(&mut UART::INSTANCE, "0x{:x}: table_lvl2.entries[{}] = {:x}", virtual_address, lvl2_index, table_lvl2.entries[lvl2_index]);
}

fn add_lvl2_mapping(virtual_address: u64, physical_address: u64, size: usize, attribute: u8, permissions: u64) {
    assert!((virtual_address % L2_PAGE_SIZE) == 0, "virtual_address not aligned");
    assert!((physical_address % L2_PAGE_SIZE) == 0, "physical_address not aligned");
    assert!((size % (L2_PAGE_SIZE as usize)) == 0, "size not aligned");

    let mut current_virtual_address: u64 = virtual_address;
    let mut current_physical_address: u64 = physical_address;
    let mut remaining_size: usize = size;

    while (remaining_size > 0) {
        unsafe {
            add_single_lvl2_mapping(current_virtual_address, current_physical_address, attribute, permissions);
        }

        current_virtual_address += L2_PAGE_SIZE;
        current_physical_address += L2_PAGE_SIZE;
        remaining_size -= L2_PAGE_SIZE as usize;
    }
}

unsafe fn get_sctlr() -> u64 {
    isb(SY);

    let mut ctrl: u64;

    asm!("mrs {sctlr}, sctlr_el2", sctlr = out(reg) ctrl, options(nostack));

    ctrl
}

unsafe fn set_sctlr(new_sctlr: u64) {
    asm!("msr sctlr_el2, {sctlr}", sctlr = in(reg) new_sctlr, options(nostack));
    asm!("ic iallu");
    dsb(SY);
    isb(SY);
}

pub unsafe fn setup() {
    // configure level 0
    for (i, entry) in (&mut LVL0_TABLE.entries[..]).iter_mut().enumerate() {
        let lvl1_table_address: u64 = &mut LVL1_TABLE[i] as *mut _ as u64;

        *entry = create_table_page_table_entry(lvl1_table_address)
    }

    // Add default mappings
    add_lvl2_mapping(0x0000000000, 0x0000000000, 0x0800000000, mem_attr::DEVICE_nGnRE, PERMISSION_RW);
    add_lvl2_mapping(0x0800000000, 0x0800000000, 0x0400000000, mem_attr::NORMAL, PERMISSION_RWX);

    dsb(SY);

    writeln!(&mut UART::INSTANCE, "Configuring MMU...").ok();

    let mair: u64 = (0xFF << (u64::from(mem_attr::NORMAL) * 8))
    | (0x00 << (u64::from(mem_attr::DEVICE_nGnRE) * 8))
    | (0x04 << (u64::from(mem_attr::DEVICE_nGnRnE) * 8));

    asm!("msr mair_el2, {mair}", mair = in(reg) mair, options(nostack));

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
        | TCR_T0SZ_48BIT;

    asm!("msr tcr_el2, {tcr}", tcr = in(reg) tcr, options(nostack));

    let ttbr = &mut LVL0_TABLE.entries[0] as *mut _ as u64;

    writeln!(&mut UART::INSTANCE, "{:x}", ttbr).ok();

    asm!("msr ttbr0_el2, {ttbr0}", ttbr0 = in(reg) ttbr, options(nostack));
    asm!("msr ttbr1_el2, {ttbr1}", ttbr1 = in(reg) ttbr, options(nostack));

    asm!("isb sy
          tlbi alle2
          isb sy
          ic iallu
          isb sy
        ");

    writeln!(&mut UART::INSTANCE, "sctrl setup").ok();

    let sctrl_old = get_sctlr();

    let sctrl_new = sctrl_old  |
                     (1 << 12) |    // I, Instruction cache enable. This is an enable bit for instruction caches at EL0 and EL1
                     (1 << 2)  |    // C, Data cache enable. This is an enable bit for data caches at EL0 and EL1
                     (1 << 0); // set M, enable MMU
    
    set_sctlr(sctrl_new);

    writeln!(&mut UART::INSTANCE, "MMU on!").ok();
}
