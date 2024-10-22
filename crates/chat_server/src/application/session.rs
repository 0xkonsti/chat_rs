use std::sync::Arc;

use chat_core::protocol::Message;
use tokio::{net::TcpStream, sync::mpsc};
use uuid::Uuid;

use super::user::User;

#[derive(Debug)]
pub struct Session {
    id: Uuid,
    //socket: Arc<TcpStream>,
    user: Option<User>,
    tx: Option<mpsc::UnboundedSender<Message>>,

    closed: bool,
}

impl Session {
    pub fn new() -> Self {
        let id = Uuid::new_v4();

        Self {
            id,
            //socket,
            user: None,
            tx: None,
            closed: false,
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
}
