#![no_std]
#![no_main]
#![feature(asm, global_asm, naked_functions, alloc_error_handler)]

#[macro_use]
extern crate alloc;

use log::info;

mod exception_vectors;
mod logger;
mod m1;
mod m1_hal;
mod m1n1;
mod mmu;
mod memory;
mod rt;
mod utils;

entry!(main);

use m1n1::UartReply;
use m1n1::ProxyReply;
use m1::uart::UART;
use embedded_hal::serial::Write;

fn main() {
    logger::init(1_500_000).expect("Logger init failed");

    info!("Hello I'm m1saka say m1saka");

    m1n1::proxy_handler();
}
