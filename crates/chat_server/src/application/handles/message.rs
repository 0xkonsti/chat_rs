use chat_core::protocol::Message;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::application::{ArcRwLock, SharedState};

pub async fn handle_direct_message_send(
    message: &Message,
    tx: mpsc::UnboundedSender<Message>,
    shared_state: ArcRwLock<SharedState>,
    session_id: Uuid,
) {
    let payload = message.payload();
    let recipient = std::str::from_utf8(&payload.get_data()[0]).unwrap().to_string();
    let message = std::str::from_utf8(&payload.get_data()[1]).unwrap().to_string();
    let sender = shared_state
        .read()
        .await
        .get_user_by_session(&session_id)
        .await
        .unwrap();

    let shared_state = shared_state.read().await;

    if let Some(session) = shared_state.get_session_by_user(&recipient).await {
        let other_session = session.read().await;
        let message = Message::direct_message_receive(&sender, &message);
        other_session.send(message).unwrap();
    } else {
        tx.send(Message::message_error(&format!("User {} is not connected", recipient)))
            .unwrap();
    }
}
