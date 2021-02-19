//! m1m1 protocol handler

use core::convert::From;
use core::convert::TryFrom;
use core::convert::TryInto;

use crate::m1::uart::UART;
use embedded_hal::serial::{Read, Write};

use log::error;
use log::warn;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
pub enum CommandId {
    NoOperation,
    Proxy,
    MemoryRead,
    MemoryWrite,
    Boot,
}

impl TryFrom<u32> for CommandId {
    type Error = &'static str;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value & 0x00FFFFFF != 0x00AA55FF {
            return Err("Invalid magic");
        }

        let raw_command_id = (value >> 24) & 0xFF;

        match raw_command_id {
            0 => Ok(CommandId::NoOperation),
            1 => Ok(CommandId::Proxy),
            2 => Ok(CommandId::MemoryRead),
            3 => Ok(CommandId::MemoryWrite),
            4 => Ok(CommandId::Boot),
            _ => Err("Unknown command"),
        }
    }
}

impl TryFrom<u8> for CommandId {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CommandId::NoOperation),
            1 => Ok(CommandId::Proxy),
            2 => Ok(CommandId::MemoryRead),
            3 => Ok(CommandId::MemoryWrite),
            4 => Ok(CommandId::Boot),
            _ => Err("Unknown command"),
        }
    }
}

impl From<CommandId> for u32 {
    fn from(value: CommandId) -> u32 {
        (value as u32) << 24 | 0x00AA55FF
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(i32)]
pub enum Status {
    Ok,
    BadCommand,
    Invalid,
    TransferError,
    ChecksumMismatch,
}

impl TryFrom<i32> for Status {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        if value & 0x00FFFFFF != 0x00AA55FF {
            return Err("Invalid magic");
        }

        let raw_command_id = (value >> 24) & 0xFF;

        match raw_command_id {
            0 => Ok(Status::Ok),
            -1 => Ok(Status::BadCommand),
            -2 => Ok(Status::Invalid),
            -3 => Ok(Status::TransferError),
            -4 => Ok(Status::ChecksumMismatch),
            _ => Err("Unknown status"),
        }
    }
}

impl From<Status> for i32 {
    fn from(value: Status) -> i32 {
        match value {
            Status::Ok => 0,
            Status::BadCommand => -1,
            Status::Invalid => -2,
            Status::TransferError => -3,
            Status::ChecksumMismatch => -4,
        }
    }
}

#[derive(Debug)]
pub struct ProxyReply {
    pub opcode: u64,
    pub status: i64,
    pub return_value: u64,
}

#[derive(Debug)]
pub enum UartReply {
    Simple {
        command_id: CommandId,
        status: Status,
    },
    Proxy {
        command_id: CommandId,
        status: Status,
        reply: ProxyReply,
    },
}

fn checksum(buffer: &[u8]) -> u32 {
    let mut sum: u32 = 0xDEADBEEF;

    for val in buffer {
        sum *= 31337;
        sum += u32::from(*val ^ 0x5A);
    }

    sum ^ 0xADDEDBAD
}

impl UartReply {
    pub const fn no_operation() -> Self {
        UartReply::Simple {
            command_id: CommandId::NoOperation,
            status: Status::Ok,
        }
    }

    pub const fn boot() -> Self {
        UartReply::Simple {
            command_id: CommandId::Boot,
            status: Status::Ok,
        }
    }

    pub fn proxy(reply: ProxyReply) -> Self {
        UartReply::Proxy {
            command_id: CommandId::Proxy,
            status: Status::Ok,
            reply,
        }
    }

    pub fn simple_error(command_id: CommandId, status: Status) -> Self {
        UartReply::Simple { command_id, status }
    }

    pub fn simple_error_from_request(request: UartRequest, status: Status) -> Self {
        UartReply::Simple {
            command_id: request.get_command_id(),
            status,
        }
    }

    pub fn to_raw_packet(&self) -> [u8; 36] {
        let mut result = [0; 36];

        let slice = &mut result[..];

        match self {
            UartReply::Simple { command_id, status } => {
                let command_id = &u32::to_le_bytes(u32::from(*command_id))[..];
                let status = &i32::to_le_bytes(i32::from(*status))[..];

                slice[..4].copy_from_slice(command_id);
                slice[4..8].copy_from_slice(status);
            }
            UartReply::Proxy {
                command_id,
                status,
                reply,
            } => {
                let command_id = &u32::to_le_bytes(u32::from(*command_id))[..];
                let status = &i32::to_le_bytes(i32::from(*status))[..];

                slice[..4].copy_from_slice(command_id);
                slice[4..8].copy_from_slice(status);

                let opcode = &u64::to_le_bytes(reply.opcode)[..];
                let proxy_status = &i64::to_le_bytes(reply.status)[..];
                let ret_value = &u64::to_le_bytes(reply.return_value)[..];

                slice[8..16].copy_from_slice(opcode);
                slice[16..24].copy_from_slice(proxy_status);
                slice[24..32].copy_from_slice(ret_value);
            }
        }

        // Update checksum
        let checksum = checksum(&slice[..32]);
        let raw_checksum = &u32::to_le_bytes(checksum)[..];

        slice[32..].copy_from_slice(raw_checksum);

        result
    }
}

impl Write<UartReply> for UART {
    type Error = ();

    fn write(&mut self, data: UartReply) -> nb::Result<(), Self::Error> {
        self.write_data(&data.to_raw_packet()[..]);
        Ok(())
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        self.wait_transmit();
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProxyRequest {
    pub opcode: u64,
    pub args: [u64; 6],
}

#[derive(Debug)]
pub enum UartRequest {
    Simple {
        command_id: CommandId,
    },
    Proxy {
        command_id: CommandId,
        request: ProxyRequest,
    },
    Memory {
        command_id: CommandId,
    },
}

impl UartRequest {
    pub fn get_command_id(&self) -> CommandId {
        match self {
            UartRequest::Simple { command_id } => *command_id,
            UartRequest::Proxy { command_id, .. } => *command_id,
            UartRequest::Memory { command_id } => *command_id,
        }
    }
}

fn read_packet_raw_command_id() -> Option<u8> {
    let mut uart = UART::INSTANCE;

    if uart.read().unwrap() != 0xFF || uart.read().unwrap() != 0x55 || uart.read().unwrap() != 0xAA
    {
        return None;
    }

    Some(uart.read().unwrap())
}

fn read_packet() -> Option<UartRequest> {
    let mut uart = UART::INSTANCE;
    let raw_command_id = read_packet_raw_command_id()?;

    let command_id = CommandId::try_from(raw_command_id);

    if command_id.is_err() {
        warn!(
            "Received invalid packet with command id: {}",
            raw_command_id
        );
        return None;
    }

    let command_id = command_id.unwrap();

    let mut raw_packet = [0x0u8; 64];

    raw_packet[0] = 0xFF;
    raw_packet[1] = 0x55;
    raw_packet[2] = 0xAA;
    raw_packet[3] = raw_command_id;

    for entry in raw_packet.iter_mut().skip(4) {
        *entry = uart.read().unwrap();
    }

    let expected_checksum: u32 = u32::from_le_bytes(raw_packet[60..64].try_into().unwrap());
    let computed_checksum: u32 = checksum(&raw_packet[..60]);

    if expected_checksum != computed_checksum {
        uart.write(UartReply::Simple {
            command_id,
            status: Status::ChecksumMismatch,
        })
        .ok();

        error!(
            "Bad checksum {:x} vs {:x}",
            expected_checksum, computed_checksum
        );

        return None;
    }

    match command_id {
        CommandId::NoOperation => Some(UartRequest::Simple { command_id }),
        CommandId::Proxy => {
            let mut args = [0x0; 6];

            for i in 0..args.len() {
                args[i] = u64::from_le_bytes(
                    raw_packet[12 + (i * core::mem::size_of::<u64>())
                        ..20 + (i * core::mem::size_of::<u64>())]
                        .try_into()
                        .unwrap(),
                );
            }

            let request = ProxyRequest {
                opcode: u64::from_le_bytes(raw_packet[4..12].try_into().unwrap()),
                args,
            };

            Some(UartRequest::Proxy {
                command_id,
                request,
            })
        }
        _ => {
            error!("Unhandled command parsing: {:?}", command_id);
            uart.write(UartReply::Simple {
                command_id,
                status: Status::BadCommand,
            })
            .ok();

            None
        }
    }
}

fn handle_packet(packet: UartRequest) -> UartReply {
    match packet {
        UartRequest::Simple { command_id } => match command_id {
            CommandId::NoOperation => UartReply::Simple {
                command_id,
                status: Status::Ok,
            },
            _ => {
                error!("Unhandled command parsing: {:?}", command_id);

                UartReply::simple_error(command_id, Status::BadCommand)
            }
        },

        _ => {
            error!("Unhandled command: {:?}", packet);

            UartReply::simple_error_from_request(packet, Status::BadCommand)
        }
    }
}

pub fn proxy_handler() {
    let mut uart = UART::INSTANCE;

    uart.write(UartReply::boot()).ok();

    loop {
        let packet = read_packet();

        if let Some(packet) = packet {
            let reply = handle_packet(packet);

            uart.write(reply).ok();
        }
    }
}
