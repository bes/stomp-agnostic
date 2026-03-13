use anyhow::anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use std::str::FromStr;
use stomp_agnostic::{
    ClientStompHandle, ClientTransport, FromServer, ReadData, ReadError, ServerResponse, ToServer,
    WriteError,
};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::{Error, Message, Utf8Bytes};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

/// The easiest way to test the client is to first start the example server and the run the client:
/// `cargo run --example stomp_server`
/// `cargo run --example stomp_client ws://127.0.0.1:8241/ws`
#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt().finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        tracing::error!("Usage: stomp_client <url>");
        std::process::exit(1);
    }
    let ws_uri = &args[1];
    let transport = make_transport(ws_uri.to_string()).await.unwrap();

    let host = transport.host();
    let mut stomp = ClientStompHandle::connect(transport, host.to_string(), None, None, Vec::new())
        .await
        .unwrap();

    let random_id = uuid::Uuid::new_v4().to_string();
    let destination = "/".to_string();
    let message = subscribe_message(destination.clone(), random_id, None);

    tracing::info!("Sending a subscribe message!");
    stomp.send_message(message).await.unwrap();

    stomp
        .as_mut_transport()
        .ws_stream
        .send(Message::Ping(Bytes::copy_from_slice(&[0_u8, 1, 2])))
        .await
        .unwrap();

    let response = stomp.read_response().await.unwrap();

    match response {
        ServerResponse::Message(from_server) => match from_server.content {
            FromServer::Connected { .. } => {
                tracing::info!("Connected: unexpected, expecting Pong");
            }
            FromServer::Message { .. } => {
                tracing::info!("Message: unexpected, expecting Pong");
            }
            FromServer::Receipt { .. } => {
                tracing::info!("Receipt: unexpected, expecting Pong");
            }
            FromServer::Error { .. } => {
                tracing::info!("Error: unexpected, expecting Pong");
            }
        },
        ServerResponse::Custom(proto) => match proto {
            WebsocketProto::Ping(bytes) => {
                tracing::info!("Got ping {bytes:?}")
            }
            WebsocketProto::Pong(bytes) => {
                tracing::info!("Got pong {bytes:?}");
            }
        },
    }

    let disconnect_message = stomp_agnostic::Message {
        content: ToServer::Disconnect { receipt: None },
        extra_headers: vec![],
    };
    stomp.send_message(disconnect_message).await.unwrap();
    match stomp.read_response().await {
        Ok(response) => {
            tracing::error!("Unexpected response {response:?}");
        }
        Err(err) => match err {
            ReadError::ConnectionClosed => {
                tracing::info!("Connection closed")
            }
            ReadError::UnexpectedMessage => {
                tracing::error!("Got an unexpected message while closing STOMP connection");
            }
            ReadError::Parser(e) => {
                tracing::error!("Got a parser error while closing STOMP connection {e:?}");
            }
            ReadError::Other(e) => {
                tracing::error!("Got a generic error while closing STOMP connection {e:?}");
            }
        },
    }
    tracing::info!("Goodbye");
}

/// We use the WebSocket (ws:// or wss://) protocol for communication.
/// STOMP and WebSocket can coexist, and we need to support at least
/// Ping/Pong for our ws protocol part.
/// This protocol definition can be expanded if needed.
#[derive(Debug)]
enum WebsocketProto {
    Ping(Bytes),
    Pong(Bytes),
}

/// An extension trait for [Transport] that requires the transport implementor to
/// provide a way to turn the transport into a [Sink] as well as getting the host name.
trait TransportExt: ClientTransport + Send + Sync {
    fn host(&self) -> String;
}

async fn make_transport(url: String) -> Result<WsTransport, Error> {
    let uri = {
        // This is a bit annoying, the URL _must_ have a "path", even if it is just a slash "/".
        // We can roundtrip the url: String -> Uri -> String -> Uri, which will normalize the Uri.
        // Disgusting (derogatory).
        // See: https://github.com/snapview/tungstenite-rs/issues/494
        Uri::from_str(&Uri::from_str(&url)?.to_string())?
    };
    let request = uri.clone().into_client_request()?;
    let (ws_stream, _) = connect_async(request).await?;

    Ok(WsTransport { uri, ws_stream })
}

/// A concrete [Transport] implementation that uses [tokio_tungstenite] for websocket communication.
struct WsTransport {
    uri: Uri,
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl TransportExt for WsTransport {
    fn host(&self) -> String {
        self.uri.host().unwrap().to_string()
    }
}

#[async_trait]
impl ClientTransport for WsTransport {
    type ProtocolSideChannel = WebsocketProto;

    async fn write(
        &mut self,
        message: stomp_agnostic::Message<ToServer>,
    ) -> Result<(), WriteError> {
        let bytes = message.into_bytes();
        // Check that bytes is valid UTF-8!
        // NOTE: This is part of the safety, do not remove!
        let _ = str::from_utf8(&bytes)?;
        let ws_message = Message::Text(
            // Safety: We checked that bytes are valid UTF-8 above
            unsafe { Utf8Bytes::from_bytes_unchecked(bytes) },
        );
        self.ws_stream
            .send(ws_message)
            .await
            .map_err(|e| WriteError::Other(anyhow!(e)))
    }

    async fn read(&mut self) -> Result<ReadData<Self::ProtocolSideChannel>, ReadError> {
        loop {
            let message = self
                .ws_stream
                .next()
                .await
                .transpose()
                .map_err(|e| ReadError::Other(anyhow!(e)))?;
            if let Some(message) = message {
                match message {
                    Message::Text(utf8_bytes) => {
                        return Ok(ReadData::Binary(Bytes::copy_from_slice(
                            utf8_bytes.as_bytes(),
                        )));
                    }
                    Message::Binary(bytes) => {
                        return Ok(ReadData::Binary(bytes));
                    }
                    Message::Ping(data) => {
                        return Ok(ReadData::Custom(WebsocketProto::Ping(data)));
                    }
                    Message::Pong(data) => {
                        return Ok(ReadData::Custom(WebsocketProto::Pong(data)));
                    }
                    Message::Close(_) => {
                        return Err(ReadError::ConnectionClosed);
                    }
                    Message::Frame(_) => {
                        return Err(ReadError::UnexpectedMessage);
                    }
                }
            }
        }
    }
}

fn subscribe_message(
    destination: String,
    id: String,
    headers: Option<Vec<(String, String)>>,
) -> stomp_agnostic::Message<ToServer> {
    // Create the basic Subscribe message
    let mut msg: stomp_agnostic::Message<ToServer> = ToServer::Subscribe {
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
