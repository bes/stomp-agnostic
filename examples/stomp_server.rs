use anyhow::anyhow;
use async_trait::async_trait;
use axum::Router;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket};
use axum::http::Method;
use axum::response::IntoResponse;
use axum::routing::get;
use bytes::Bytes;
use stomp_agnostic::{
    ClientData, FromServer, ReadData, ReadError, ServerStompHandle, ServerTransport, ToServer,
    WriteError,
};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

/// The easiest way to test the server is to start it and then run the example client:
/// `cargo run --example stomp_server`
/// `cargo run --example stomp_client ws://127.0.0.1:8241/ws`
#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt().finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    let listener = match TcpListener::bind("127.0.0.1:8241").await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("{err:?}");
            return;
        }
    };

    let router = Router::new()
        .route("/ws", get(websocket_handler))
        .layer(CorsLayer::new().allow_methods([Method::GET, Method::POST]));

    tracing::info!("Websocket server listening at ws://127.0.0.1:8241/ws");

    match axum::serve(listener, router.into_make_service()).await {
        Ok(_) => tracing::info!("Axum exited cleanly"),
        Err(err) => tracing::warn!("Axum exited with an error {}", err),
    };
}

async fn websocket_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_failed_upgrade(|error| tracing::error!("Error upgrading websocket: {}", error))
        .on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let transport = WsStomp { socket };

    let mut stomp_handle = match ServerStompHandle::wait_for_connection(
        transport,
        // Handle login
        Box::new(|_login, _pass| true),
        // Handle session ids
        Box::new(|| None),
    )
    .await
    {
        Ok(stomp_handle) => stomp_handle,
        Err(err) => {
            tracing::error!("{err:?}");
            return;
        }
    };

    loop {
        match stomp_handle.read_data().await {
            Ok(data) => match data {
                ClientData::Message(message) => match message.content {
                    ToServer::Connect { .. } => {
                        tracing::warn!("Connect message received? Should not happen.");
                    }
                    ToServer::Send { .. } => {
                        tracing::warn!("Send is unsupported.");
                    }
                    ToServer::Subscribe {
                        destination, id, ..
                    } => {
                        tracing::info!("Subscribe request {id} {destination}");
                    }
                    ToServer::Unsubscribe { .. } => {
                        tracing::warn!("Unsubscribe is unsupported.");
                    }
                    ToServer::Ack { .. } => {
                        tracing::warn!("Ack is unsupported.");
                    }
                    ToServer::Nack { .. } => {
                        tracing::warn!("Nack is unsupported.");
                    }
                    ToServer::Begin { .. } => {
                        tracing::warn!("Begin is unsupported.");
                    }
                    ToServer::Commit { .. } => {
                        tracing::warn!("Commit is unsupported.");
                    }
                    ToServer::Abort { .. } => {
                        tracing::warn!("Abort is unsupported.");
                    }
                    ToServer::Disconnect { .. } => {
                        send_close_message(stomp_handle.into_transport().socket, 1111, "close")
                            .await;
                        break;
                    }
                },
                ClientData::Custom(proto) => match proto {
                    WebsocketProto::Ping(ping) => {
                        match stomp_handle
                            .as_mut_transport()
                            .socket
                            .send(Message::Pong(ping))
                            .await
                        {
                            Ok(_) => {
                                tracing::info!("Sent ping");
                            }
                            Err(err) => {
                                tracing::error!("Error when reading data {err:?}");
                                break;
                            }
                        };
                    }
                    WebsocketProto::Pong(pong) => {
                        match stomp_handle
                            .as_mut_transport()
                            .socket
                            .send(Message::Ping(pong))
                            .await
                        {
                            Ok(_) => {
                                tracing::info!("Sent pong");
                            }
                            Err(err) => {
                                tracing::error!("Error when reading data {err:?}");
                                break;
                            }
                        };
                    }
                },
            },
            Err(err) => {
                tracing::error!("Error when reading data {err:?}");
                break;
            }
        }
    }
}

async fn send_close_message(mut socket: WebSocket, code: u16, reason: &str) {
    _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

#[derive(Debug)]
pub enum WebsocketProto {
    Ping(Bytes),
    Pong(Bytes),
}

struct WsStomp {
    socket: WebSocket,
}

#[async_trait]
impl ServerTransport for WsStomp {
    type ProtocolSideChannel = WebsocketProto;

    async fn write(
        &mut self,
        message: stomp_agnostic::Message<FromServer>,
    ) -> Result<(), WriteError> {
        let bytes = message.into_bytes();
        let utf8_bytes: Utf8Bytes = bytes.try_into()?;
        self.socket
            .send(Message::Text(utf8_bytes))
            .await
            .map_err(|e| WriteError::Other(anyhow!(e)))?;
        Ok(())
    }

    async fn read(&mut self) -> Result<ReadData<Self::ProtocolSideChannel>, ReadError> {
        if let Some(message) = self
            .socket
            .recv()
            .await
            .transpose()
            .map_err(|e| ReadError::Other(anyhow!(e)))?
        {
            match message {
                Message::Text(utf8_bytes) => Ok(ReadData::Binary(Bytes::copy_from_slice(
                    utf8_bytes.as_bytes(),
                ))),
                Message::Binary(bytes) => Ok(ReadData::Binary(bytes)),
                Message::Ping(data) => Ok(ReadData::Custom(WebsocketProto::Ping(data))),
                Message::Pong(data) => Ok(ReadData::Custom(WebsocketProto::Pong(data))),
                Message::Close(_) => Err(ReadError::ConnectionClosed),
            }
        } else {
            Err(ReadError::ConnectionClosed)
        }
    }
}
