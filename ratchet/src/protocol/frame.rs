use crate::protocol::HeaderFlags;
use bytes::BufMut;
use bytes::{Bytes, BytesMut};
use derive_more::Display;
use nanorand::{WyRand, RNG};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::convert::TryFrom;
use thiserror::Error;

const U16_MAX: usize = u16::MAX as usize;

#[derive(Display)]
pub enum OpCode {
    #[display(fmt = "{}", _0)]
    DataCode(DataCode),
    #[display(fmt = "{}", _0)]
    ControlCode(ControlCode),
}

impl From<OpCode> for u8 {
    fn from(op: OpCode) -> Self {
        match op {
            OpCode::DataCode(code) => code as u8,
            OpCode::ControlCode(code) => code as u8,
        }
    }
}

#[derive(Display)]
pub enum DataCode {
    #[display(fmt = "Continuation")]
    Continuation = 0,
    #[display(fmt = "Text")]
    Text = 1,
    #[display(fmt = "Binary")]
    Binary = 2,
}

#[derive(Display)]
pub enum ControlCode {
    #[display(fmt = "Close")]
    Close = 8,
    #[display(fmt = "Ping")]
    Ping = 9,
    #[display(fmt = "Pong")]
    Pong = 10,
}

#[derive(Debug, Error)]
pub enum OpCodeParseErr {
    #[error("Reserved OpCode: `{0}`")]
    Reserved(u8),
    #[error("Invalid OpCode: `{0}`")]
    Invalid(u8),
}

impl TryFrom<u8> for OpCode {
    type Error = OpCodeParseErr;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(OpCode::DataCode(DataCode::Continuation)),
            1 => Ok(OpCode::DataCode(DataCode::Text)),
            2 => Ok(OpCode::DataCode(DataCode::Binary)),
            r @ 3..=7 => Err(OpCodeParseErr::Reserved(r)),
            8 => Ok(OpCode::ControlCode(ControlCode::Close)),
            9 => Ok(OpCode::ControlCode(ControlCode::Ping)),
            10 => Ok(OpCode::ControlCode(ControlCode::Pong)),
            r @ 11..=15 => Err(OpCodeParseErr::Reserved(r)),
            e => Err(OpCodeParseErr::Invalid(e)),
        }
    }
}

pub struct Frame {
    data: Bytes,
    kind: FrameKind,
}

pub enum FrameKind {
    Text,
    Binary,
    Continuation,
    Ping,
    Pong,
    Close(Option<CloseReason>),
}

pub struct CloseReason {
    code: CloseCode,
    description: Option<String>,
}

pub enum CloseCode {
    GoingAway,
}

pub enum Message {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

impl AsMut<[u8]> for Message {
    fn as_mut(&mut self) -> &mut [u8] {
        todo!()
    }
}

pub fn write_into<A>(
    dst: &mut BytesMut,
    mut flags: HeaderFlags,
    opcode: OpCode,
    mut payload: A,
    mask: u32,
) where
    A: AsMut<[u8]>,
{
    let mut payload = payload.as_mut();
    let mut length = payload.len();
    let mut masked = flags.is_masked();

    let (second, mut offset) = if masked {
        flags.set(HeaderFlags::MASKED, false);
        apply_mask(mask, &mut payload);

        (0x80, 6)
    } else {
        (0x0, 2)
    };

    if length >= U16_MAX {
        offset += 8;
    } else if length > 125 {
        offset += 2;
    }

    let additional = if masked {
        payload.len() + offset
    } else {
        offset
    };

    dst.reserve(additional);
    let first = flags.bits | u8::from(opcode);

    if length < 126 {
        dst.extend_from_slice(&[first, second | length as u8]);
    } else if length <= U16_MAX {
        dst.extend_from_slice(&[first, second | 126]);
        dst.put_u16(length as u16);
    } else {
        dst.extend_from_slice(&[first, second | 127]);
        dst.put_u64(length as u64);
    };

    if masked {
        dst.put_u32_le(mask as u32);
    }

    dst.extend_from_slice(payload);
}

// todo speed up with an XOR lookup table
pub fn apply_mask(mask: u32, bytes: &mut [u8]) {
    let mask: [u8; 4] = mask.to_be_bytes();

    for i in 0..bytes.len() {
        bytes[i] ^= mask[i & 0x3]
    }
}
