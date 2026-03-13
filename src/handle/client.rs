use crate::transport::client::{BufferedTransport, ClientTransport, ServerResponse};
use crate::{FromServer, Message, ReadError, ToServer, WriteError};
use anyhow::anyhow;
use std::fmt::Debug;

/// A handle that reads and writes STOMP messages given an implementation of [ClientTransport].
pub struct ClientStompHandle<T>
where
    T: ClientTransport,
    T::ProtocolSideChannel: Debug,
{
    transport: BufferedTransport<T>,
}

impl<T> ClientStompHandle<T>
where
    T: ClientTransport,
    T::ProtocolSideChannel: Debug,
{
    /// Creates a new [ClientStompHandle] for your code to interface with.
    /// Requires an implementation of [ClientTransport].
    pub async fn connect(
        transport: T,
        virtualhost: String,
        login: Option<String>,
        passcode: Option<String>,
        headers: Vec<(String, String)>,
    ) -> anyhow::Result<ClientStompHandle<T>> {
        let transport = client_handshake(
            BufferedTransport::new(transport),
            virtualhost.clone(),
            login,
            passcode,
            headers,
        )
        .await?;

        Ok(ClientStompHandle { transport })
    }

    /// Send a STOMP message through the underlying transport
    pub async fn send_message(&mut self, message: Message<ToServer>) -> Result<(), WriteError> {
        self.transport.send(message).await
    }

    /// Read a STOMP message from the underlying transport
    pub async fn read_response(
        &mut self,
    ) -> Result<ServerResponse<T::ProtocolSideChannel>, ReadError> {
        self.transport.next().await
    }

    /// Consume the [ClientStompHandle] to get the original [ClientTransport] back.
    pub fn into_transport(self) -> T {
        self.transport.into_transport()
    }

    /// Get a mutable reference to the transport, to be able to handle e.g. WebSocket Ping/Pong
    pub fn as_mut_transport(&mut self) -> &mut T {
        self.transport.as_mut_inner()
    }
}

/// Performs the STOMP protocol handshake with the server
///
/// This function sends a CONNECT frame to the server and waits for
/// a CONNECTED response. If the server responds with anything else,
/// the handshake is considered failed.
async fn client_handshake<T>(
    mut transport: BufferedTransport<T>,
    virtualhost: String,
    login: Option<String>,
    passcode: Option<String>,
    headers: Vec<(String, String)>,
) -> anyhow::Result<BufferedTransport<T>>
where
    T: ClientTransport,
    T::ProtocolSideChannel: Debug,
{
    // Convert custom headers to the binary format expected by the protocol
    let extra_headers = headers
        .iter()
        .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
        .collect();

    // Create the CONNECT message
    let connect = Message {
        content: ToServer::Connect {
            accept_version: "1.2".into(),
            host: virtualhost,
            login,
            passcode,
            heartbeat: None,
        },
        extra_headers,
    };

    // Send the message to the server
    transport.send(connect).await?;

    // Receive and process the server's reply
    let response = transport.next().await?;

    match response {
        ServerResponse::Message(msg) => {
            // Check if the reply is a CONNECTED frame
            if let FromServer::Connected { .. } = msg.content {
                Ok(transport)
            } else {
                Err(anyhow!("Unexpected response: {msg:?}"))
            }
        }
        ServerResponse::Custom(custom) => Err(anyhow!("Unexpected response: {custom:?}")),
    }
}
