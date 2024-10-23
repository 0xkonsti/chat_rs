// use std::error::Error;

use chrono::prelude::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};

macro_rules! error_string {
    ($e:expr) => {
        if let Err(e) = $e {
            return Err(e.to_string());
        }
    };
}

const HEADER_START: u16 = 0x5918;
const VERSION: u8 = 0x01;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageType {
    Empty = 0x00,
    Ack = 0x01,
    Nack = 0x02,
    Disconnect = 0x03,
    Heartbeat = 0x04,

    // Authentification
    Auth = 0x10,
    AuthCreate = 0x11,
    AuthSuccess = 0x12,
    AuthFailure = 0x13,

    ServerDebugLog = 0x20,

    Break = 0xff,
}

#[derive(Debug, Clone)]
struct Header {
    version: u8,
    message_type: MessageType,
}

#[derive(Debug, Clone)]
struct PayloadField {
    field_length: u32,
    field_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Payload {
    count: u32,
    fields: Vec<PayloadField>,
}

#[derive(Debug, Clone)]
pub struct Message {
    header: Header,
    payload: Payload,
    checksum: u32,
}

#[derive(Debug)]
pub struct MessageBuilder {
    header: Header,
    payload: Payload,
}

impl MessageType {
    pub fn from(value: u8) -> Self {
        match value {
            0x00 => MessageType::Empty,
            0x01 => MessageType::Ack,
            0x02 => MessageType::Nack,
            0x03 => MessageType::Disconnect,
            0x04 => MessageType::Heartbeat,

            0x10 => MessageType::Auth,
            0x11 => MessageType::AuthCreate,
            0x12 => MessageType::AuthSuccess,
            0x13 => MessageType::AuthFailure,

            0x20 => MessageType::ServerDebugLog,

            0xff => MessageType::Break,

            _ => MessageType::Empty,
        }
    }
}

impl Header {
    const fn from_message_type(message_type: MessageType) -> Self {
        Header {
            version: VERSION,
            message_type,
        }
    }
}

impl PayloadField {
    fn new(field_data: Vec<u8>) -> Self {
        PayloadField {
            field_length: field_data.len() as u32,
            field_data,
        }
    }
}

impl Into<String> for PayloadField {
    fn into(self) -> String {
        String::from_utf8_lossy(&self.field_data).to_string()
    }
}

impl Payload {
    const EMPTY: Payload = Payload {
        count: 0,
        fields: Vec::new(),
    };

    fn add_field(&mut self, field_data: Vec<u8>) {
        self.fields.push(PayloadField::new(field_data));
        self.count += 1;
    }

    fn add_fields(&mut self, fields: Vec<Vec<u8>>) {
        for field in fields {
            self.add_field(field);
        }
    }

    fn checksum(&self) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        for field in &self.fields {
            hasher.update(&field.field_data);
        }
        hasher.finalize()
    }

    pub fn get_data(&self) -> Vec<Vec<u8>> {
        self.fields.iter().map(|field| field.field_data.clone()).collect()
    }
}

impl Default for Payload {
    fn default() -> Self {
        Payload {
            count: 0,
            fields: Vec::new(),
        }
    }
}

impl Message {
    pub const ACK: Message = Message {
        header: Header::from_message_type(MessageType::Ack),
        payload: Payload::EMPTY,
        checksum: 0,
    };
    pub const BREAK: Message = Message {
        header: Header::from_message_type(MessageType::Break),
        payload: Payload::EMPTY,
        checksum: 0,
    };
    pub const DISCONNECT: Message = Message {
        header: Header::from_message_type(MessageType::Disconnect),
        payload: Payload::EMPTY,
        checksum: 0,
    };
    pub const EMPTY: Message = Message {
        header: Header::from_message_type(MessageType::Empty),
        payload: Payload::EMPTY,
        checksum: 0,
    };
    pub const NACK: Message = Message {
        header: Header::from_message_type(MessageType::Nack),
        payload: Payload::EMPTY,
        checksum: 0,
    };
    pub const SERVER_DEBUG_LOG: Message = Message {
        header: Header::from_message_type(MessageType::ServerDebugLog),
        payload: Payload::EMPTY,
        checksum: 0,
    };

    pub fn heartbeat() -> Self {
        let mut payload = Payload::default();
        payload.add_field(Local::now().to_rfc3339().as_bytes().to_vec());
        let checksum = payload.checksum();

        Message {
            header: Header::from_message_type(MessageType::Heartbeat),
            payload,
            checksum,
        }
    }

    pub fn auth(username: &str, password: &str) -> Self {
        let mut payload = Payload::default();
        payload.add_field(username.as_bytes().to_vec());
        payload.add_field(password.as_bytes().to_vec());
        let checksum = payload.checksum();

        Message {
            header: Header::from_message_type(MessageType::Auth),
            payload,
            checksum,
        }
    }

    pub fn auth_create(username: &str, password: &str) -> Self {
        let mut payload = Payload::default();
        payload.add_field(username.as_bytes().to_vec());
        payload.add_field(password.as_bytes().to_vec());
        let checksum = payload.checksum();

        Message {
            header: Header::from_message_type(MessageType::AuthCreate),
            payload,
            checksum,
        }
    }

    pub fn auth_success() -> Self {
        Message {
            header: Header::from_message_type(MessageType::AuthSuccess),
            payload: Payload::EMPTY,
            checksum: 0,
        }
    }

    pub fn auth_fail(error: &str) -> Self {
        let mut payload = Payload::default();
        payload.add_field(error.as_bytes().to_vec());
        let checksum = payload.checksum();

        Message {
            header: Header::from_message_type(MessageType::AuthFailure),
            payload,
            checksum,
        }
    }

    pub fn is(&self, message_type: MessageType) -> bool {
        self.header.message_type == message_type
    }

    pub fn message_type(&self) -> MessageType {
        self.header.message_type
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub async fn send(&self, stream: &mut OwnedWriteHalf) -> Result<(), String> {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&HEADER_START.to_be_bytes());
        buf.push(self.header.version);
        buf.push(self.header.message_type.clone() as u8);
        buf.extend_from_slice(&self.payload.count.to_be_bytes());

        for field in &self.payload.fields {
            buf.extend_from_slice(&field.field_length.to_be_bytes());
            buf.extend_from_slice(&field.field_data);
        }

        buf.extend_from_slice(&self.checksum.to_be_bytes());

        error_string!(stream.write_all(&buf).await);

        Ok(())
    }

    pub async fn receive(stream: &mut OwnedReadHalf) -> Result<Self, String> {
        let mut buf = [0u8; 1];
        error_string!(stream.read_exact(&mut buf).await);
        let version = buf[0];
        if version != VERSION {
            return Err("Invalid version".into());
        }

        let mut buf = [0u8; 1];
        error_string!(stream.read_exact(&mut buf).await);
        let message_type = MessageType::from(buf[0]);

        let mut builder = MessageBuilder::new(message_type);

        let mut buf = [0u8; 4];
        error_string!(stream.read_exact(&mut buf).await);
        let payload_count = u32::from_be_bytes(buf);

        for _ in 0..payload_count {
            let mut buf = [0u8; 4];
            error_string!(stream.read_exact(&mut buf).await);
            let field_length = u32::from_be_bytes(buf);

            let mut field_data = vec![0u8; field_length as usize];
            error_string!(stream.read_exact(&mut field_data).await);

            builder = builder.with_field(field_data);
        }

        let mut buf = [0u8; 4];
        error_string!(stream.read_exact(&mut buf).await);
        let checksum = u32::from_be_bytes(buf);

        if checksum != builder.payload.checksum() {
            return Err("Invalid checksum".into());
        }

        Ok(builder.build())
    }

    pub async fn has_header_start(stream: &mut OwnedReadHalf) -> bool {
        let mut buffer = [0u8; 2];
        match stream.read_exact(&mut buffer).await {
            Ok(_) => u16::from_be_bytes(buffer) == HEADER_START,
            Err(_) => false,
        }
    }
}

impl MessageBuilder {
    pub fn new(message_type: MessageType) -> Self {
        MessageBuilder {
            header: Header {
                version: VERSION,
                message_type,
            },
            payload: Payload::default(),
        }
    }

    pub fn with_field(mut self, field_data: Vec<u8>) -> Self {
        self.payload.add_field(field_data);
        self
    }

    pub fn with_fields(mut self, fields: Vec<Vec<u8>>) -> Self {
        self.payload.add_fields(fields);
        self
    }

    pub fn build(self) -> Message {
        let checksum = self.payload.checksum();

        Message {
            header: self.header,
            payload: self.payload,
            checksum,
        }
    }
}
