use std::sync::Arc;

use argon2::Config;
use chat_core::{
    constants::{HOST, PORT},
    protocol::{Message, MessageType},
};
use chrono::{DateTime, Utc};
use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::mpsc,
};
use uuid::Uuid;

use super::{ArcRwLock, SharedState};
use crate::application::{session::Session, user::User};

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
        session.update_heartbeat(None);

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
                if let Err(e) = message.send(&mut writer).await {
                    tracing::error!("Error sending message: {}", e);
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
                        MessageType::Heartbeat => {
                            shared_state
                                .read()
                                .await
                                .update_heartbeat(
                                    session_id,
                                    Some(
                                        DateTime::parse_from_rfc3339(
                                            std::str::from_utf8(&message.payload().get_data()[0]).unwrap(),
                                        )
                                        .unwrap()
                                        .with_timezone(&Utc),
                                    ),
                                )
                                .await;
                        }
                        MessageType::Auth => {
                            if shared_state.read().await.is_authenticated(session_id).await {
                                tx.send(Message::NACK).unwrap();
                                continue;
                            }
                            let data = message.payload().get_data();
                            let username = std::str::from_utf8(&data[0]).unwrap();

                            let user = shared_state.read().await.get_user(username).cloned();
                            if let Some(user) = user {
                                if user.session_id().is_some() {
                                    tx.send(Message::auth_fail("User already logged in")).unwrap();
                                    continue;
                                }
                                if argon2::verify_encoded(&user.pw_hash(), &data[1]).unwrap() {
                                    shared_state
                                        .write()
                                        .await
                                        .authenticate(session_id, user.name().to_string())
                                        .await;
                                    tx.send(Message::auth_success()).unwrap();
                                    continue;
                                }
                            }
                            tx.send(Message::auth_fail("Invalid username or password")).unwrap();
                        }
                        MessageType::AuthCreate => {
                            if shared_state.read().await.is_authenticated(session_id).await {
                                tx.send(Message::NACK).unwrap();
                                continue;
                            }
                            let data = message.payload().get_data();
                            let username = std::str::from_utf8(&data[0]).unwrap();

                            if shared_state.read().await.get_user(username).is_none() {
                                let password = std::str::from_utf8(&data[1]).unwrap();
                                let config = Config::default();
                                // TODO: create a random salt for each user
                                let hash = argon2::hash_encoded(password.as_bytes(), b"randomsalt", &config).unwrap();

                                let username_string = username.to_string();
                                let user = User::new(username, hash);

                                shared_state.write().await.add_user(username.to_string(), user);
                                shared_state
                                    .write()
                                    .await
                                    .authenticate(session_id, username_string)
                                    .await;
                                tx.send(Message::auth_success()).unwrap();
                                continue;
                            }

                            tx.send(Message::auth_fail("User already exists")).unwrap();
                        }
                        MessageType::ServerDebugLog => {
                            tracing::debug!("{:#?}", shared_state.read().await);
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

            if let Err(e) = tx.send(Message::heartbeat()) {
                tracing::warn!("Error sending heartbeat: {}", e);
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }
}
