use std::convert::TryInto;

use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};
use crc::{crc16, Hasher16};

use crate::commands::{IncomingCommandS, OutgoingCommand};

pub trait Parser {
    fn read_cmd<R: ReadBytesExt>(input: R) -> Result<IncomingCommandS, ()>;
    fn write_cmd<W: WriteBytesExt>(output: W, cmd: OutgoingCommand) -> Result<(), ()>;
}

pub enum APIv1 {}

const START_BYTE_V1: u8 = 0x3E;

impl Parser for APIv1 {
    fn read_cmd<R: ReadBytesExt>(input: R) -> Result<IncomingCommandS, ()> {
        todo!();
    }

    fn write_cmd<W: WriteBytesExt>(output: W, cmd: OutgoingCommand) -> Result<(), ()> {
        let len = cmd.payload.len().try_into().unwrap();
        // TODO: Be safe about IDs. Maybe use num_traits ToPrimitive?
        let id = cmd.id as u8;
        let checksum = cmd.payload.iter().fold(0, |acc, x| (acc + x) % 256);

        output.write_u8(START_BYTE_V1);
        output.write_u8(id);
        output.write_u8(len);
        output.write_u8((id + len) % 256);
        output.write(cmd.payload);
        output.write_u8(checksum);

        Ok(())
    }
}

pub enum APIv2 {}

const START_BYTE_V2: u8 = 0x24;

impl Parser for APIv2 {
    fn read_cmd<R: ReadBytesExt>(input: R) -> Result<IncomingCommandS, ()> {
        todo!();
    }

    fn write_cmd<W: WriteBytesExt>(output: W, cmd: OutgoingCommand) -> Result<(), ()> {
        let len = cmd.payload.len().try_into().unwrap();
        // TODO: Be safe about IDs
        let id = cmd.id as u8;
        let checksum = {
            let mut digest = crc16::Digest::new(0x8005);
            digest.write(cmd.payload);
            digest.sum16()
        };

        output.write_u8(START_BYTE_V2);
        output.write_u8(id);
        output.write_u8(len);
        output.write_u8((id + len) % 256);
        output.write(cmd.payload);
        output.write_u16::<LittleEndian>(checksum);

        Ok(())
    }
}
