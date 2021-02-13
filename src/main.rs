#![no_std]
#![no_main]
#![feature(asm, global_asm, naked_functions, alloc_error_handler)]

#[macro_use]
extern crate alloc;

use log::info;

mod exception_vectors;
mod logger;
mod m1;
mod mmu;
mod memory;
mod rt;
mod utils;

entry!(main);

fn main() {
    logger::init(1_500_000);

    info!("Hello I'm m1saka say m1saka");
}
