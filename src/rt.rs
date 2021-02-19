#![allow(clippy::empty_loop)]

use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr;

use crate::m1::uart::UART;

use crate::exception_vectors;
use crate::memory;
use crate::mmu;

#[macro_export]
macro_rules! entry {
    ($path:path) => {
        #[export_name = "main"]
        pub unsafe fn __main() -> () {
            // type check the given path
            let f: fn() -> () = $path;

            f()
        }
    };
}

#[panic_handler]
fn panic(panic_info: &PanicInfo<'_>) -> ! {
    let mut uart = UART::INSTANCE;

    writeln!(&mut uart, "PANIC: {}\r", panic_info).ok();

    loop {}
}

#[alloc_error_handler]
fn allocation_error(_: core::alloc::Layout) -> ! {
    let mut uart = UART::INSTANCE;

    writeln!(&mut uart, "Memory exhausting").ok();

    loop {}
}

extern "C" {
    static mut __start_bss__: u8;
    static mut __end_bss__: u8;
    static _stack_bottom: u8;
    static _stack_top: u8;
}

#[link_section = ".text.crt0"]
#[naked]
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    asm!(
        "
        b trampoline
        .word _DYNAMIC - _start
        ",
        options(noreturn),
    )
}

#[naked]
#[no_mangle]
pub unsafe extern "C" fn trampoline() -> ! {
    asm!(
        "
        adrp x0, _stack_top
        add x0, x0, #:lo12:_stack_top
        mov sp, x0
        adrp x0, _start
        bl relocate_self

        adrp x0, __bss_start__
        add x0, x0, #:lo12:__bss_start__
        adrp x1, __bss_end__
        add x1, x1, #:lo12:__bss_end__
        bl clean_bss
        bl _start_with_stack
        ",
        options(noreturn),
    )
}

const DT_NULL: isize = 0;
const DT_RELA: isize = 7;
const DT_RELAENT: isize = 9;
const DT_RELACOUNT: isize = 0x6ffffff9;
const DT_REL: isize = 17;
const DT_RELENT: isize = 19;
const DT_RELCOUNT: isize = 0x6ffffffa;

const R_AARCH64_RELATIVE: usize = 0x403;

#[repr(C)]
#[derive(Debug)]
struct ElfDyn {
    tag: isize,
    val: usize,
}

#[repr(C)]
struct ElfRel {
    offset: usize,
    info: usize,
}

#[repr(C)]
struct ElfRela {
    offset: usize,
    info: usize,
    addend: isize,
}

#[no_mangle]
pub unsafe extern "C" fn relocate_self(aslr_base: *mut u8) -> u32 {
    let mut dynamic =
        aslr_base.offset(*(aslr_base.offset(4) as *const u32) as isize) as *mut ElfDyn;

    let mut rela_offset = None;
    let mut rela_entry_size = 0;
    let mut rela_count = 0;

    let mut rel_offset = None;
    let mut rel_entry_size = 0;
    let mut rel_count = 0;

    while (*dynamic).tag != DT_NULL {
        match (*dynamic).tag {
            DT_RELA => {
                rela_offset = Some((*dynamic).val);
            }
            DT_RELAENT => {
                rela_entry_size = (*dynamic).val;
            }
            DT_REL => {
                rel_offset = Some((*dynamic).val);
            }
            DT_RELENT => {
                rel_entry_size = (*dynamic).val;
            }
            DT_RELACOUNT => {
                rela_count = (*dynamic).val;
            }
            DT_RELCOUNT => {
                rel_count = (*dynamic).val;
            }
            _ => {}
        }
        dynamic = dynamic.offset(1);
    }

    if let Some(rela_offset) = rela_offset {
        if rela_entry_size != core::mem::size_of::<ElfRela>() {
            return 2;
        }
        let rela_base = (aslr_base.add(rela_offset)) as *mut ElfRela;

        for i in 0..rela_count {
            let rela = rela_base.add(i);

            let reloc_type = (*rela).info & 0xffffffff;

            if reloc_type == R_AARCH64_RELATIVE {
                *(aslr_base.add((*rela).offset) as *mut *mut ()) =
                    aslr_base.offset((*rela).addend) as _;
            } else {
                return 4;
            }
        }
    }

    if let Some(rel_offset) = rel_offset {
        if rel_entry_size != core::mem::size_of::<ElfRel>() {
            return 3;
        }

        let rel_base = (aslr_base.add(rel_offset)) as *mut ElfRel;

        for i in 0..rel_count {
            let rel = rel_base.add(i);

            let reloc_type = (*rel).info & 0xffffffff;

            if let R_AARCH64_RELATIVE = reloc_type {
                let ptr = aslr_base.add((*rel).offset) as *mut usize;
                *ptr += aslr_base as usize;
            } else {
                return 4;
            }
        }
    }
    0
}

#[no_mangle]
unsafe extern "C" fn clean_bss(start_bss: *mut u8, end_bss: *mut u8) {
    ptr::write_bytes(
        start_bss,
        0,
        end_bss as *const _ as usize - start_bss as *const _ as usize,
    );
}

#[no_mangle]
pub unsafe extern "C" fn _start_with_stack() -> ! {
    memory::setup();
    exception_vectors::setup();
    mmu::setup();

    // Call user entry point
    extern "Rust" {
        fn main() -> ();
    }

    main();

    loop {}
}
