use argon2::Config;
use chat_core::protocol::Message;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::application::{user::User, ArcRwLock, SharedState};

pub async fn handle_auth(
    message: &Message,
    tx: mpsc::UnboundedSender<Message>,
    shared_state: ArcRwLock<SharedState>,
    session_id: Uuid,
) {
    if shared_state.read().await.is_authenticated(session_id).await {
        tx.send(Message::NACK).unwrap();
        return;
    }
    let data = message.payload().get_data();
    let username = std::str::from_utf8(&data[0]).unwrap();

    let user = shared_state.read().await.get_user(username).cloned();
    if let Some(user) = user {
        if user.session_id().is_some() {
            tx.send(Message::auth_fail("User already logged in")).unwrap();
            return;
        }
        if argon2::verify_encoded(&user.pw_hash(), &data[1]).unwrap() {
            shared_state
                .write()
                .await
                .authenticate(session_id, user.name().to_string())
                .await;

            tx.send(Message::auth_success()).unwrap();
            return;
        }
    }
    tx.send(Message::auth_fail("Invalid username or password")).unwrap();
}

pub async fn handle_auth_create(
    message: &Message,
    tx: mpsc::UnboundedSender<Message>,
    shared_state: ArcRwLock<SharedState>,
    session_id: Uuid,
) {
    if shared_state.read().await.is_authenticated(session_id).await {
        tx.send(Message::NACK).unwrap();
        return;
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
        return;
    }

    tx.send(Message::auth_fail("User already exists")).unwrap();
}
