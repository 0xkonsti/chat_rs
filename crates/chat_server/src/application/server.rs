use std::sync::Arc;

use chat_core::{
    constants::{HOST, PORT},
    protocol::{Message, MessageType},
};
use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::mpsc,
};
use uuid::Uuid;

use super::{ArcRwLock, SharedState};
use crate::application::session::Session;

#[derive(Debug)]
pub struct Server {}
impl Server {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn serve(&self, shared_state: ArcRwLock<SharedState>) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Starting server on {}:{}", HOST, PORT);
        let listener = TcpListener::bind((HOST, PORT)).await?;
        tracing::info!("Server started");

        loop {
            let (socket, addr) = listener.accept().await?;

            tracing::info!("Accepted connection from {}", addr);

            tokio::spawn(Self::handle_connection(socket, Arc::clone(&shared_state)));
        }

        // tracing::info!("Shutting down server");
        // Ok(())
    }

    async fn handle_connection(socket: TcpStream, shared_state: ArcRwLock<SharedState>) {
        let socket_addr = socket.peer_addr().unwrap();
        let (reader, writer) = socket.into_split();
        let (tx, rx) = mpsc::unbounded_channel::<Message>();

        //let mut session = Session::new(Arc::clone(&socket));
        let mut session = Session::new();
        let session_id = session.id();
        session.set_channel(tx.clone());

        shared_state
            .write()
            .await
            .add_session(session.id(), Arc::new(tokio::sync::RwLock::new(session)));

        let send_h = tokio::spawn(Self::handle_send(writer, rx, Arc::clone(&shared_state), session_id));
        let recv_h = tokio::spawn(Self::handle_receive(
            reader,
            tx.clone(),
            Arc::clone(&shared_state),
            session_id,
        ));

        let hb_h = tokio::spawn(Self::handle_heartbeat(
            tx.clone(),
            Arc::clone(&shared_state),
            session_id,
        ));

        tx.send(Message::PING).unwrap();
        tx.send(Message::PONG).unwrap();
        tx.send(Message::ACK).unwrap();

        send_h.await.unwrap();
        recv_h.await.unwrap();
        hb_h.await.unwrap();

        tracing::info!("Closed connection from {}", socket_addr);
    }

    async fn handle_send(
        mut writer: OwnedWriteHalf,
        mut rx: mpsc::UnboundedReceiver<Message>,
        shared_state: ArcRwLock<SharedState>,
        session_id: Uuid,
    ) {
        loop {
            if !shared_state.read().await.is_active_session(session_id).await {
                break;
            }

            if let Some(message) = rx.recv().await {
                tracing::info!("Sending message: {:?}", message.message_type());
                if !message.send(&mut writer).await.is_ok() {
                    tracing::error!("Error sending message");
                    Self::handle_disconnect(shared_state.clone(), session_id).await;
                }
                if message.is(MessageType::Disconnect) {
                    Self::handle_disconnect(shared_state.clone(), session_id).await;
                }
            }
        }
    }

    async fn handle_receive(
        mut reader: OwnedReadHalf,
        tx: mpsc::UnboundedSender<Message>,
        shared_state: ArcRwLock<SharedState>,
        session_id: Uuid,
    ) {
        loop {
            if !shared_state.read().await.is_active_session(session_id).await {
                break;
            }

            if !Message::has_header_start(&mut reader).await {
                continue;
            }

            let message = Message::receive(&mut reader).await;
            match message {
                Ok(message) => {
                    tracing::info!("Received message: {:?}", message.message_type());
                    match message.message_type() {
                        MessageType::Disconnect => {
                            tx.send(Message::DISCONNECT).unwrap();
                            break;
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    tracing::error!("Error receiving message: {}", e);
                    break;
                }
            }
        }
    }

    async fn handle_disconnect(shared_state: ArcRwLock<SharedState>, session_id: Uuid) {
        shared_state.write().await.close_session(session_id).await;
    }

    async fn handle_heartbeat(
        tx: mpsc::UnboundedSender<Message>,
        shared_state: ArcRwLock<SharedState>,
        session_id: Uuid,
    ) {
        loop {
            if !shared_state.read().await.is_active_session(session_id).await {
                break;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

            if let Err(e) = tx.send(Message::heartbeat()) {
                tracing::warn!("Error sending heartbeat: {}", e);
            }
        }
    }
}
