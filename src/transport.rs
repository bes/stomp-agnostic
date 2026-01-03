use crate::{FromServer, Message, ToServer, frame};
use async_trait::async_trait;
use bytes::{Buf, Bytes, BytesMut};
use std::fmt::Debug;
use std::str::Utf8Error;
use thiserror::Error;
use winnow::Partial;
use winnow::error::{ContextError, ErrMode};
use winnow::stream::Offset;

#[async_trait]
pub trait Transport: Send + Sync {
    /// A side channel to shuffle arbitrary data that is not part of the STOMP communication,
    /// e.g. WebSocket Ping/Pong.
    type ProtocolSideChannel;

    async fn write(&mut self, message: Message<ToServer>) -> Result<(), WriteError>;
    async fn read(&mut self) -> Result<ReadResponse<Self::ProtocolSideChannel>, ReadError>;
}

/// A response coming down the line from the transport layer. When the transport layer is
/// e.g. WebSocket, custom data such as Ping/Pong can be handled separately from STOMP data
/// by using the `Custom` variant.
#[derive(Debug)]
pub enum ReadResponse<T> {
    Binary(Bytes),
    Custom(T),
}

/// A parsed response, either a [Message] coming from the server, or a custom protocol signal
/// in the `Custom` variant.
#[derive(Debug)]
pub enum Response<T>
where
    T: Debug,
{
    Message(Message<FromServer>),
    Custom(T),
}

#[derive(Error, Debug)]
pub enum WriteError {
    #[error("Utf8Error")]
    Utf8Error(#[from] Utf8Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum ReadError {
    /// This is the most important [Transport] error to take care of - when the connection has been
    /// closed, this is the only error that shall be returned when reading. This is so that
    /// implementors / users of the trait can handle this case consistently.
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Unexpected message")]
    UnexpectedMessage,
    #[error("Parser error")]
    Parser(ErrMode<ContextError>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub(crate) struct BufferedTransport<T>
where
    T: Transport,
    T::ProtocolSideChannel: Debug,
{
    transport: T,
    buffer: BytesMut,
}

impl<T> BufferedTransport<T>
where
    T: Transport,
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

    fn decode(&mut self) -> Result<Option<Message<FromServer>>, ReadError> {
        // Create a partial view of the buffer for parsing
        let buf = &mut Partial::new(self.buffer.chunk());

        // Attempt to parse a frame from the buffer
        let item = match frame::parse_frame(buf) {
            Ok(frame) => Message::<FromServer>::from_frame(frame),
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

    pub(crate) async fn send(&mut self, message: Message<ToServer>) -> Result<(), WriteError> {
        self.transport.write(message).await
    }

    pub(crate) async fn next(&mut self) -> Result<Response<T::ProtocolSideChannel>, ReadError> {
        loop {
            let response = self.transport.read().await?;
            match response {
                ReadResponse::Binary(buffer) => {
                    self.append(buffer);
                }
                ReadResponse::Custom(custom) => {
                    return Ok(Response::Custom(custom));
                }
            }

            if let Some(message) = self.decode()? {
                return Ok(Response::Message(message));
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
