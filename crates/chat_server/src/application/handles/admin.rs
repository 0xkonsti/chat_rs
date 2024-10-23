use chat_core::protocol::Message;

use crate::application::{ArcRwLock, SharedState};

pub async fn handle_server_shutdown(message: &Message, shared_state: ArcRwLock<SharedState>) {
    let data = message.payload().get_data();
    let timeout = u64::from_be_bytes(data[0].clone().try_into().unwrap());

    let read_shared_state = shared_state.read().await;
    let sessions = read_shared_state.sessions();

    for (id, session) in sessions.iter() {
        if let Err(e) = session.read().await.send(Message::server_shutdown_warning(timeout)) {
            tracing::warn!("Error sending server shutdown warning to session {}: {}", id, e);
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(timeout)).await;

    for (id, session) in sessions.iter() {
        if let Err(e) = session.read().await.send(Message::DISCONNECT) {
            tracing::warn!("Error sending server shutdown to session {}: {}", id, e);
        }
    }

    drop(read_shared_state);
    shared_state.write().await.shutdown().await;
}
