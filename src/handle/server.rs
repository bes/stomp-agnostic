use crate::transport::server::{BufferedTransport, ClientData, ServerTransport};
use crate::{FromServer, Message, ReadError, ToServer, WriteError};
use std::fmt::{Debug, Formatter};

/// A handle that reads and writes STOMP messages given an implementation of [ServerTransport].
pub struct ServerStompHandle<T>
where
    T: ServerTransport,
    T::ProtocolSideChannel: Debug,
{
    transport: BufferedTransport<T>,
}

impl<T> ServerStompHandle<T>
where
    T: ServerTransport,
    T::ProtocolSideChannel: Debug,
{
    /// Creates a new [ServerStompHandle] for your code to interface with.
    /// Requires an implementation of [ServerTransport].
    pub async fn wait_for_connection(
        transport: T,
        handle_login: Box<dyn Fn(Option<String>, Option<String>) -> bool + Send>,
        create_session: Box<dyn Fn() -> Option<String> + Send>,
    ) -> Result<ServerStompHandle<T>, ClientHandshakeError<T>> {
        match client_handshake(
            BufferedTransport::new(transport),
            handle_login,
            create_session,
        )
        .await
        {
            Ok(transport) => Ok(ServerStompHandle { transport }),
            Err(error) => Err(error),
        }
    }

    /// Send a STOMP message through the underlying transport
    pub async fn send_message(&mut self, message: Message<FromServer>) -> Result<(), WriteError> {
        self.transport.send(message).await
    }

    /// Read a STOMP message from the underlying transport
    pub async fn read_data(&mut self) -> Result<ClientData<T::ProtocolSideChannel>, ReadError> {
        self.transport.next().await
    }

    /// Consume the [ServerStompHandle] to get the original [ServerTransport] back.
    pub fn into_transport(self) -> T {
        self.transport.into_transport()
    }

    /// Get a mutable reference to the transport, to be able to handle e.g. WebSocket Ping/Pong
    pub fn as_mut_transport(&mut self) -> &mut T {
        self.transport.as_mut_inner()
    }
}

pub struct ClientHandshakeError<T>
where
    T: ServerTransport,
    T::ProtocolSideChannel: Debug,
{
    pub error: String,
    pub transport: T,
}

impl<T> Debug for ClientHandshakeError<T>
where
    T: ServerTransport,
    T::ProtocolSideChannel: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClientHandshakeError: {}", self.error)
    }
}

async fn client_handshake<T>(
    mut transport: BufferedTransport<T>,
    handle_login: Box<dyn Fn(Option<String>, Option<String>) -> bool + Send>,
    create_session: Box<dyn Fn() -> Option<String> + Send>,
) -> Result<BufferedTransport<T>, ClientHandshakeError<T>>
where
    T: ServerTransport,
    T::ProtocolSideChannel: Debug,
{
    let connect_message = match transport.next().await {
        Ok(message) => message,
        Err(err) => {
            return Err(ClientHandshakeError {
                error: format!("Connect message error {err:?}"),
                transport: transport.into_transport(),
            });
        }
    };

    match connect_message {
        ClientData::Message(msg) => {
            if let ToServer::Connect {
                accept_version,
                host,
                login,
                passcode,
                heartbeat,
            } = msg.content
            {
                let accept_versions: Vec<&str> = accept_version.split(',').collect();
                if !accept_versions.contains(&"1.2") {
                    return Err(ClientHandshakeError {
                        error: format!(
                            "We only support STOMP 1.2 but client only accepts {accept_version:?}"
                        ),
                        transport: transport.into_transport(),
                    });
                }
                if !handle_login(login, passcode) {
                    return Err(ClientHandshakeError {
                        error: "Login was not accepted".to_string(),
                        transport: transport.into_transport(),
                    });
                }
                // TODO: Host / virtual hosting is not implemented yet
                tracing::info!("Virtual hosting is not implemented, client asked for {host}");
                if let Some(heartbeat) = heartbeat
                    && (heartbeat.0 > 0 || heartbeat.1 > 0)
                {
                    tracing::warn!(
                        "We do not support heartbeat yet, client requested ({},{})",
                        heartbeat.0,
                        heartbeat.1
                    );
                }
                let session = create_session();
                let connected = Message {
                    content: FromServer::Connected {
                        version: "1.2".to_string(),
                        session,
                        server: None,
                        heartbeat: None,
                    },
                    extra_headers: vec![],
                };
                match transport.send(connected).await {
                    Ok(_) => {}
                    Err(err) => {
                        return Err(ClientHandshakeError {
                            error: format!("Send error {err:?}"),
                            transport: transport.into_transport(),
                        });
                    }
                }
                Ok(transport)
            } else {
                Err(ClientHandshakeError {
                    error: format!("Unexpected connect message: {msg:?}"),
                    transport: transport.into_transport(),
                })
            }
        }
        ClientData::Custom(custom) => Err(ClientHandshakeError {
            error: format!("Unexpected connect message: {custom:?}"),
            transport: transport.into_transport(),
        }),
    }
}
