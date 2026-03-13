use bytes::Bytes;
use std::str::Utf8Error;
use thiserror::Error;
use winnow::error::{ContextError, ErrMode};

pub(crate) mod client;
pub(crate) mod server;

/// Data coming down the line from the transport layer. When the transport layer is
/// e.g. WebSocket, custom data such as Ping/Pong can be handled separately from STOMP data
/// by using the `Custom` variant.
#[derive(Debug)]
pub enum ReadData<T> {
    Binary(Bytes),
    Custom(T),
}

#[derive(Error, Debug)]
pub enum ReadError {
    /// This is the most important error to take care of - when the connection has been
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

#[derive(Error, Debug)]
pub enum WriteError {
    #[error("Utf8Error")]
    Utf8Error(#[from] Utf8Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
