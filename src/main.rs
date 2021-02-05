#![no_std]
#![no_main]
#![feature(asm, naked_functions)]

use log::info;

mod logger;
mod m1;
mod rt;

entry!(main);

fn main() {
    logger::init(115_200);

    loop {
        info!("Hello I'm m1saka say m1saka");
    }
}
