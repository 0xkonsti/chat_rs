use std::{collections::HashMap, error::Error, sync::Arc};

use tokio::sync::RwLock;

mod server;
mod session;
mod user;

use server::Server;
use session::Session;
use user::User;
use uuid::Uuid;

const TRACING_LEVEL: tracing::Level = tracing::Level::DEBUG;
type ArcRwLock<T> = Arc<RwLock<T>>;

#[derive(Debug)]
struct SharedState {
    users: HashMap<String, User>,
    sessions: HashMap<Uuid, ArcRwLock<Session>>,
}

#[derive(Debug)]
pub struct Application {
    server: server::Server,
    shared_state: ArcRwLock<SharedState>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            sessions: HashMap::new(),
        }
    }

    pub fn add_user(&mut self, name: String, user: User) {
        self.users.insert(name, user);
    }

    pub fn add_session(&mut self, id: Uuid, session: ArcRwLock<Session>) {
        self.sessions.insert(id, session);
    }

    pub fn get_user(&self, name: &str) -> Option<&User> {
        self.users.get(name)
    }

    pub fn get_session(&self, id: Uuid) -> Option<ArcRwLock<Session>> {
        self.sessions.get(&id).cloned()
    }

    pub async fn remove_session(&mut self, id: Uuid) {
        if !self.sessions.contains_key(&id) {
            return;
        }
        let session = self.sessions.get(&id).unwrap().read().await;
        let user = session.user();
        if let Some(user) = user {
            self.users.get_mut(user.name()).unwrap().remove_session_id();
        }
        drop(session);
        self.sessions.remove(&id);
    }

    pub async fn is_active_session(&self, id: Uuid) -> bool {
        self.sessions.contains_key(&id) && !self.sessions[&id].read().await.is_closed()
    }

    pub async fn close_session(&mut self, id: Uuid) {
        if let Some(session) = self.sessions.get(&id) {
            session.write().await.close();
        }

        self.sessions.remove(&id);

        tracing::debug!("Closed session {}", id);
    }
}

impl Application {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        use tracing_subscriber::fmt::format::FmtSpan;
        tracing_subscriber::fmt()
            .with_max_level(TRACING_LEVEL)
            .compact()
            .with_span_events(FmtSpan::FULL)
            .init();

        Ok(Self::default())
    }

    pub async fn run(&self) -> Result<(), Box<dyn Error>> {
        tracing::info!("Running application");

        self.server.serve(Arc::clone(&self.shared_state)).await?;

        tracing::info!("Application finished");
        Ok(())
    }
}

impl Default for Application {
    fn default() -> Self {
        Self {
            server: Server::new(),
            shared_state: Arc::new(RwLock::new(SharedState::new())),
        }
    }
}
