#![no_std]
#![no_main]
#![feature(asm, global_asm, naked_functions, alloc_error_handler)]
#![allow(dead_code)]

extern crate alloc;

use log::info;

mod exception_vectors;
mod logger;
mod m1;
mod m1_hal;
mod m1n1;
mod memory;
mod mmu;
mod rt;
mod utils;

entry!(main);

fn main() {
    logger::init(1_500_000).expect("Logger init failed");

    info!("Hello I'm m1saka say m1saka");

    m1n1::proxy_handler();
}
