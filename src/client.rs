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

    pub async fn send_message(&mut self, message: Message<ToServer>) -> anyhow::Result<()> {
        self.transport.send(message).await
    }

    pub async fn read_message(&mut self) -> anyhow::Result<Message<FromServer>> {
        self.transport.next().await
    }
}

/// Create an instance of a [ToServer::Subscribe] message that can be sent to the STOMP server
/// to start a subscription.
pub fn subscribe_message(
    destination: String,
    id: String,
    headers: Option<Vec<(String, String)>>,
) -> Message<ToServer> {
    // Create the basic Subscribe message
    let mut msg: Message<ToServer> = ToServer::Subscribe {
        destination,
        id,
        ack: None,
    }
    .into();

    // Add any custom headers
    if let Some(headers) = headers {
        msg.extra_headers = headers
            .iter()
            .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
            .collect();
    }

    msg
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
