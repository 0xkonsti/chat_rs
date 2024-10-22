use chat_core::protocol::Message;
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::user::User;

#[derive(Debug)]
pub struct Session {
    id: Uuid,
    user: Option<User>,
    tx: Option<mpsc::UnboundedSender<Message>>,
    last_heartbeat: Option<DateTime<Utc>>,

    closed: bool,
}

impl Session {
    pub fn new() -> Self {
        let id = Uuid::new_v4();

        Self {
            id,
            user: None,
            tx: None,
            closed: false,
            last_heartbeat: None,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    pub fn set_user(&mut self, user: User) {
        self.user = Some(user);
    }

    pub fn set_channel(&mut self, tx: mpsc::UnboundedSender<Message>) {
        self.tx = Some(tx);
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn last_heartbeat(&self) -> Option<DateTime<Utc>> {
        self.last_heartbeat
    }

    pub fn update_heartbeat(&mut self, heartbeat: Option<DateTime<Utc>>) {
        if let Some(heartbeat) = heartbeat {
            self.last_heartbeat = Some(heartbeat);
        } else {
            self.last_heartbeat = Some(Utc::now());
        }
    }
}
