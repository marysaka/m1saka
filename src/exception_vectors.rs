use core::fmt::Write;

use crate::m1::uart::UART;

use crate::utils;

global_asm!(
    "
    .macro  vector_entry  label
    .align  7
    b       \\label
    .endm

    .macro  vector_not_handled
    .align  7
    b       _unhandled_vector
    .endm

    .section .vectors, \"ax\"
    .align  11
    .global vector_table;
    vector_table:
        /* Current EL with SP0 */
        vector_entry _start_with_stack
        vector_not_handled
        vector_not_handled
        vector_not_handled

        .align 9
        /* Current EL with SPx */
        vector_entry _current_elx_sync
        vector_not_handled
        vector_not_handled
        vector_not_handled

        .align 9
        /* Lower EL exception to Current EL (AArch64) */
        vector_not_handled
        vector_not_handled
        vector_not_handled
        vector_not_handled

        .align 9
        /* Lower EL exception to Current EL (AArch32) */
        vector_not_handled
        vector_not_handled
        vector_not_handled
        vector_not_handled
    "
);

pub fn set_vbar(vbar: u64) {
    unsafe {
        match utils::get_current_el() {
            1 => asm!("msr vbar_el1, {vbar}", vbar = in(reg) vbar, options(nostack)),
            2 => asm!("msr vbar_el2, {vbar}", vbar = in(reg) vbar, options(nostack)),
            3 => asm!("msr vbar_el3, {vbar}", vbar = in(reg) vbar, options(nostack)),
            _ => unimplemented!(),
        }
    }
}

pub fn setup() {
    extern "C" {
        static vector_table: u64;
    }

    let vbar = unsafe { &vector_table as *const _ as u64 };
    set_vbar(vbar);
}

global_asm!(
    "
    .macro  push, xreg1, xreg2
        stp     \\xreg1, \\xreg2, [sp, #-16]!
    .endm

    .macro  pop, xreg1, xreg2
        ldp     \\xreg1, \\xreg2, [sp], #16
    .endm

    .macro __save_generic_registers
        push    x29, x30
        push    x27, x28
        push    x25, x26
        push    x23, x24
        push    x21, x22
        push    x19, x20
        push    x17, x18
        push    x15, x16
        push    x13, x14
        push    x11, x12
        push    x9,  x10
        push    x7,  x8
        push    x5,  x6
        push    x3,  x4
        push    x1,  x2
    .endm

    .macro __restore_generic_registers
        pop    x1,  x2
        pop    x3,  x4
        pop    x5,  x6
        pop    x7,  x8
        pop    x9,  x10
        pop    x11, x12
        pop    x13, x14
        pop    x15, x16
        pop    x17, x18
        pop    x19, x20
        pop    x21, x22
        pop    x23, x24
        pop    x25, x26
        pop    x27, x28
        pop    x29, x30
    .endm

    .macro __save_el_registers
        mrs    x11, CurrentEL
        cmp    x11, 0xc
        b.eq   3f
        cmp    x11, 0x8
        b.eq   2f
        cmp    x11, 0x4
        b.eq   1f
    3:
        mrs     x20, esr_el3
        push    x20, x0
        mrs     x0, elr_el3
        mrs     x1, spsr_el3
        push    x0, x1
        mrs     x0, far_el3
        push    x0, x0
        b 0f
    2:
        mrs     x20, esr_el2
        push    x20, x0
        mrs     x0, elr_el2
        mrs     x1, spsr_el2
        push    x0, x1
        mrs     x0, far_el2
        push    x0, x0
        b 0f
    1:
        mrs     x20, esr_el1
        push    x20, x0
        mrs     x0, elr_el1
        mrs     x1, spsr_el1
        push    x0, x1
        mrs     x0, far_el1
        push    x0, x0
        b 0f
    0:
    .endm

    .macro __restore_el_registers
        pop    x0, x0

        pop    x0, x1

        mrs    x11, CurrentEL
        cmp    x11, 0xc
        b.eq   3f
        cmp    x11, 0x8
        b.eq   2f
        cmp    x11, 0x4
        b.eq   1f

    3:
        msr    elr_el2, x0
        msr    spsr_el2, x1
        b 0f
    2:
        msr    elr_el2, x0
        msr    spsr_el2, x1
        b 0f
    1:
        msr    elr_el1, x0
        msr    spsr_el1, x1
        b 0f
    0:
        pop    x20, x0
    .endm
    "
);

#[naked]
#[no_mangle]
unsafe extern "C" fn _unhandled_vector() -> ! {
    asm!(
        "
        __save_generic_registers
        __save_el_registers
        mov x0, sp
        bl unhandled_vector
        __restore_el_registers
        __restore_generic_registers
        eret
        ",
        options(noreturn),
    )
}

#[naked]
#[no_mangle]
unsafe extern "C" fn _current_elx_sync() -> ! {
    asm!(
        "
        __save_generic_registers
        __save_el_registers
        mov x0, sp
        bl current_elx_sync
        __restore_el_registers
        __restore_generic_registers
        eret
        ",
        options(noreturn),
    )
}

#[repr(C)]
struct ExceptionInfo {
    far_duplicate: u64,
    far: u64,
    pc: u64,
    cpsr: u64,
    esr: u64,
    x: [u64; 31],
}

unsafe fn dump_exception(exception: &mut ExceptionInfo) {
    let mut uart = UART::INSTANCE;

    writeln!(&mut uart, "Fault address:\t{:20x}\r", exception.far).ok();
    writeln!(&mut uart, "Register dump:\r").ok();
    writeln!(&mut uart, "PC:\t{:20x}\t", exception.pc).ok();
    writeln!(&mut uart, "CPSR:\t{:20x}\t", exception.cpsr).ok();
    writeln!(&mut uart, "ESR:\t{:20x}\r", exception.esr).ok();

    for (index, value) in exception.x.iter_mut().enumerate() {
        write!(&mut uart, "X{}:\t{:20x}\t", index, *value).ok();

        if (index % 3) == 0 {
            writeln!(&mut uart, "\r").ok();
        }
    }
}

#[no_mangle]
unsafe extern "C" fn unhandled_vector(exception: &mut ExceptionInfo) {
    let mut uart = UART::INSTANCE;
    writeln!(&mut uart, "\r").ok();
    writeln!(
        &mut uart,
        "Unhandled vector ({})\r",
        get_exception_type_elx(exception.esr)
    )
    .ok();
    writeln!(
        &mut uart,
        "Instruction Fault name: {}\r",
        get_instruction_fault_name(exception.esr)
    )
    .ok();

    dump_exception(exception);

    loop {}
}

pub fn get_exception_type_elx(esr: u64) -> &'static str {
    let exception_class = esr >> 26;

    match exception_class {
        0x18 => "Configurable trap",
        0x22 => "PC alignment exception",
        0x25 => "Data abort",
        0x26 => "Stack alignment exception",
        0x2f => "Serror",
        0x30 => "Debug exception",
        _ => "Unknown exception",
    }
}

pub fn get_instruction_fault_name(esr: u64) -> &'static str {
    let exception_class = esr & 0x1f;

    match exception_class {
        0b000000 => "Address size fault in TTBR0 or TTBR1",
        0b000101 => "Translation fault, 1st level",
        0b000110 => "Translation fault, 2nd level",
        0b000111 => "Translation fault, 3rd level",
        0b001001 => "Access flag fault, 1st level",
        0b001010 => "Access flag fault, 2nd level",
        0b001011 => "Access flag fault, 3rd level",
        0b001101 => "Permission fault, 1st level",
        0b001110 => "Permission fault, 2nd level",
        0b001111 => "Permission fault, 3rd level",
        0b010000 => "Synchronous external abort",
        0b011000 => "Synchronous parity error on memory access",
        0b010101 => "Synchronous external abort on translation table walk, 1st level",
        0b010110 => "Synchronous external abort on translation table walk, 2nd level",
        0b010111 => "Synchronous external abort on translation table walk, 3rd level",
        0b011101 => {
            "Synchronous parity error on memory access on translation table walk, 1st level"
        }
        0b011110 => {
            "Synchronous parity error on memory access on translation table walk, 2nd level"
        }
        0b011111 => {
            "Synchronous parity error on memory access on translation table walk, 3rd level"
        }
        0b100001 => "Alignment fault",
        0b100010 => "Debug event",
        _ => "Unknown instruction fault",
    }
}

#[no_mangle]
unsafe extern "C" fn current_elx_sync(exception: &mut ExceptionInfo) {
    let mut uart = UART::INSTANCE;
    writeln!(&mut uart, "\r").ok();
    writeln!(
        &mut uart,
        "Sync ELX Exception ({})\r",
        get_exception_type_elx(exception.esr)
    )
    .ok();
    dump_exception(exception);

    loop {}
}
