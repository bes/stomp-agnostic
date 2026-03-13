use crate::frame::parse_frame;
use crate::transport::{ReadData, ReadError, WriteError};
use crate::{FromServer, Message, ToServer};
use async_trait::async_trait;
use bytes::{Buf, Bytes, BytesMut};
use std::fmt::Debug;
use winnow::Partial;
use winnow::error::ErrMode;
use winnow::stream::Offset;

#[async_trait]
pub trait ServerTransport {
    /// A side channel to shuffle arbitrary data that is not part of the STOMP communication,
    /// e.g. WebSocket Ping/Pong.
    type ProtocolSideChannel;

    async fn write(&mut self, message: Message<FromServer>) -> Result<(), WriteError>;
    async fn read(&mut self) -> Result<ReadData<Self::ProtocolSideChannel>, ReadError>;
}

/// A parsed response, either a [Message] coming from the server, or a custom protocol signal
/// in the `Custom` variant.
#[derive(Debug)]
pub enum ClientData<T>
where
    T: Debug,
{
    Message(Message<ToServer>),
    Custom(T),
}

pub(crate) struct BufferedTransport<T>
where
    T: ServerTransport,
    T::ProtocolSideChannel: Debug,
{
    transport: T,
    buffer: BytesMut,
}

impl<T> BufferedTransport<T>
where
    T: ServerTransport,
    T::ProtocolSideChannel: Debug,
{
    pub(crate) fn new(transport: T) -> Self {
        Self {
            transport,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    fn append(&mut self, data: Bytes) {
        self.buffer.extend_from_slice(&data);
    }

    fn decode(&mut self) -> Result<Option<Message<ToServer>>, ReadError> {
        // Create a partial view of the buffer for parsing
        let buf = &mut Partial::new(self.buffer.chunk());

        // Attempt to parse a frame from the buffer
        let item = match parse_frame(buf) {
            Ok(frame) => Message::<ToServer>::from_frame(frame),
            // Need more data
            Err(ErrMode::Incomplete(_)) => return Ok(None),
            Err(e) => return Err(ReadError::Parser(e)),
        };

        // Calculate how many bytes were consumed
        let len = buf.offset_from(&Partial::new(self.buffer.chunk()));

        // Advance the buffer past the consumed bytes
        self.buffer.advance(len);

        // Return the parsed message (or error)
        item.map_err(|e| e.into()).map(Some)
    }

    pub(crate) async fn send(&mut self, message: Message<FromServer>) -> Result<(), WriteError> {
        self.transport.write(message).await
    }

    pub(crate) async fn next(&mut self) -> Result<ClientData<T::ProtocolSideChannel>, ReadError> {
        loop {
            let response = self.transport.read().await?;
            match response {
                ReadData::Binary(buffer) => {
                    self.append(buffer);
                }
                ReadData::Custom(custom) => {
                    return Ok(ClientData::Custom(custom));
                }
            }

            if let Some(message) = self.decode()? {
                return Ok(ClientData::Message(message));
            }
        }
    }

    pub(crate) fn into_transport(self) -> T {
        self.transport
    }

    pub(crate) fn as_mut_inner(&mut self) -> &mut T {
        &mut self.transport
    }
}
