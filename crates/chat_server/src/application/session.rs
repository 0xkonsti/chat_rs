use chat_core::protocol::{Message, MessageType};
use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum AccessLevel {
    Guest,
    User,
    Admin,
}

#[derive(Debug)]
pub struct Session {
    id: Uuid,
    user: Option<String>,
    access_level: AccessLevel,
    tx: Option<mpsc::UnboundedSender<Message>>,
    last_heartbeat: Option<DateTime<Utc>>,

    closed: bool,
}

impl AccessLevel {
    const ADMIN_ACCESS_GROUP: &[MessageType] = &[MessageType::ServerDebugLog];
    const GUEST_ACCESS_GROUP: &[MessageType] = &[
        MessageType::AuthCreate,
        MessageType::Auth,
        MessageType::Heartbeat,
        MessageType::Disconnect,
    ];
    const USER_ACCESS_GROUP: &[MessageType] = &[];

    pub fn can_access(&self, message_type: &MessageType) -> bool {
        match self {
            AccessLevel::Admin => {
                Self::ADMIN_ACCESS_GROUP.contains(message_type)
                    || Self::User.can_access(message_type)
                    || Self::Guest.can_access(message_type)
            }
            AccessLevel::User => Self::USER_ACCESS_GROUP.contains(message_type) || Self::Guest.can_access(message_type),
            AccessLevel::Guest => Self::GUEST_ACCESS_GROUP.contains(message_type),
        }
    }
}

impl Session {
    pub fn new() -> Self {
        let id = Uuid::new_v4();

        Self {
            id,
            user: None,
            access_level: AccessLevel::Guest,
            tx: None,
            closed: false,
            last_heartbeat: None,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn user(&self) -> Option<&String> {
        self.user.as_ref()
    }

    pub fn set_user(&mut self, user: String) {
        self.user = Some(user);
    }

    pub fn access_level(&self) -> &AccessLevel {
        &self.access_level
    }

    pub fn set_access_level(&mut self, access_level: AccessLevel) {
        self.access_level = access_level;
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
