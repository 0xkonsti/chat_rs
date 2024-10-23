use chat_core::protocol::Message;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{ArcRwLock, SharedState};

pub mod admin;
pub mod auth;
pub mod message;

pub async fn handle_heartbeat(message: &Message, shared_state: ArcRwLock<SharedState>, session_id: Uuid) {
    shared_state
        .read()
        .await
        .update_heartbeat(
            session_id,
            Some(
                DateTime::parse_from_rfc3339(std::str::from_utf8(&message.payload().get_data()[0]).unwrap())
                    .unwrap()
                    .with_timezone(&Utc),
            ),
        )
        .await;
}
