use crate::{FromServer, Message, ToServer, frame};
use anyhow::bail;
use async_trait::async_trait;
use bytes::{Buf, Bytes, BytesMut};
use std::any::Any;
use winnow::Partial;
use winnow::error::ErrMode;
use winnow::stream::Offset;

#[async_trait]
pub trait Transport: Any + Send + Sync {
    async fn write(&mut self, message: Message<ToServer>) -> anyhow::Result<()>;
    async fn read(&mut self) -> anyhow::Result<Bytes>;
}

pub(crate) struct BufferedTransport {
    transport: Box<dyn Transport>,
    buffer: BytesMut,
}

impl BufferedTransport {
    pub(crate) fn new(transport: Box<dyn Transport>) -> Self {
        Self {
            transport,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    fn append(&mut self, data: Bytes) {
        self.buffer.extend_from_slice(&data);
    }

    fn decode(&mut self) -> anyhow::Result<Option<Message<FromServer>>> {
        // Create a partial view of the buffer for parsing
        let buf = &mut Partial::new(self.buffer.chunk());

        // Attempt to parse a frame from the buffer
        let item = match frame::parse_frame(buf) {
            Ok(frame) => Message::<FromServer>::from_frame(frame),
            Err(ErrMode::Incomplete(_)) => return Ok(None), // Need more data
            Err(e) => bail!("Parse failed: {:?}", e),       // Parsing error
        };

        // Calculate how many bytes were consumed
        let len = buf.offset_from(&Partial::new(self.buffer.chunk()));

        // Advance the buffer past the consumed bytes
        self.buffer.advance(len);

        // Return the parsed message (or error)
        item.map(Some)
    }

    pub(crate) async fn send(&mut self, message: Message<ToServer>) -> anyhow::Result<()> {
        self.transport.write(message).await
    }

    pub(crate) async fn next(&mut self) -> anyhow::Result<Message<FromServer>> {
        loop {
            let buf = self.transport.read().await?;
            self.append(buf);
            if let Some(message) = self.decode()? {
                return Ok(message);
            }
        }
    }

    pub(crate) fn into_transport(self) -> Box<dyn Transport> {
        self.transport
    }
}
