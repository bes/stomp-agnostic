use crate::transport::{BufferedTransport, Transport};
use crate::{FromServer, Message, ToServer};
use anyhow::anyhow;

/// A handle that reads and writes STOMP messages given an implementation of [Transport].
pub struct StompHandle {
    transport: BufferedTransport,
}

impl StompHandle {
    /// Creates a new [StompHandle] for your code to interface with.
    /// Requires an implementation of [Transport].
    pub async fn connect(
        transport: Box<dyn Transport>,
        virtualhost: String,
        login: Option<String>,
        passcode: Option<String>,
        headers: Vec<(String, String)>,
    ) -> anyhow::Result<StompHandle> {
        let transport = client_handshake(
            BufferedTransport::new(transport),
            virtualhost.clone(),
            login,
            passcode,
            headers,
        )
        .await?;

        Ok(StompHandle { transport })
    }

    /// Send a STOMP message through the underlying transport
    pub async fn send_message(&mut self, message: Message<ToServer>) -> anyhow::Result<()> {
        self.transport.send(message).await
    }

    /// Read a STOMP message from the underlying transport
    pub async fn read_message(&mut self) -> anyhow::Result<Message<FromServer>> {
        self.transport.next().await
    }

    /// Consume the [StompHandle] to get the original [Transport] back.
    /// You can then use [Any](std::any::Any) downcasting to convert it back to your own type.
    ///
    /// Example:
    /// ```rust,ignore
    /// match (&mut *transport as &mut dyn Any).downcast_mut::<MyTransportImpl>() {
    ///     Some(my_transport) => {
    ///         // Use my_transport
    ///         my_transport.write.send(...
    ///     }
    ///     None => {
    ///         // Oops, should not happen
    ///     }
    /// };
    /// ```
    pub fn into_transport(self) -> Box<dyn Transport> {
        self.transport.into_transport()
    }
}

/// Performs the STOMP protocol handshake with the server
///
/// This function sends a CONNECT frame to the server and waits for
/// a CONNECTED response. If the server responds with anything else,
/// the handshake is considered failed.
async fn client_handshake(
    mut transport: BufferedTransport,
    virtualhost: String,
    login: Option<String>,
    passcode: Option<String>,
    headers: Vec<(String, String)>,
) -> anyhow::Result<BufferedTransport> {
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
    let msg = transport.next().await?;

    // Check if the reply is a CONNECTED frame
    if let FromServer::Connected { .. } = msg.content {
        Ok(transport)
    } else {
        Err(anyhow!("unexpected reply: {:?}", msg))
    }
}
