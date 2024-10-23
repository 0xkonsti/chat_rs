use std::{collections::HashMap, error::Error, sync::Arc};

use tokio::sync::{mpsc, RwLock};

mod handles;
mod server;
mod session;
mod user;

use server::Server;
use session::{AccessLevel, Session};
use user::User;
use uuid::Uuid;

const TRACING_LEVEL: tracing::Level = tracing::Level::DEBUG;
type ArcRwLock<T> = Arc<RwLock<T>>;

#[derive(Debug)]
struct SharedState {
    users: HashMap<String, User>,
    sessions: HashMap<Uuid, ArcRwLock<Session>>,
    shutdown_tx: Option<mpsc::Sender<bool>>,
}

#[derive(Debug)]
pub struct Application {
    server: server::Server,
    shared_state: ArcRwLock<SharedState>,
}

impl SharedState {
    pub fn new() -> Self {
        let mut users = HashMap::new();

        let mut luffy_admin = User::new(
            "luffy",
            "$argon2id$v=19$m=19456,t=2,p=1$cmFuZG9tc2FsdA$jDQwPD4k6mPV4oT/0Y4M2nhVSGDxpbbJaxIbNYc84rU".to_string(),
        );

        luffy_admin.set_access_level(AccessLevel::Admin);

        // add Admin user
        users.insert("luffy".to_string(), luffy_admin);

        Self {
            users,
            sessions: HashMap::new(),
            shutdown_tx: None,
        }
    }

    pub fn add_user(&mut self, name: String, user: User) {
        self.users.insert(name, user);
    }

    pub fn add_session(&mut self, id: Uuid, session: ArcRwLock<Session>) {
        self.sessions.insert(id, session);
    }

    pub fn set_shutdown_tx(&mut self, tx: mpsc::Sender<bool>) {
        self.shutdown_tx = Some(tx);
    }

    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            tx.send(true).await.unwrap();
        }
    }

    pub fn get_user(&self, name: &str) -> Option<&User> {
        self.users.get(name)
    }

    pub fn sessions(&self) -> &HashMap<Uuid, ArcRwLock<Session>> {
        &self.sessions
    }

    pub async fn remove_session(&mut self, id: Uuid) {
        if !self.sessions.contains_key(&id) {
            return;
        }
        let session = self.sessions.get(&id).unwrap().read().await;
        let user = session.user();
        if let Some(user) = user {
            self.users.get_mut(user).unwrap().remove_session_id();
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
            self.remove_session(id).await;
            tracing::debug!("Closed session {}", id);
        }
    }

    pub async fn update_heartbeat(&self, id: Uuid, heartbeat: Option<chrono::DateTime<chrono::Utc>>) {
        if let Some(session) = self.sessions.get(&id) {
            session.write().await.update_heartbeat(heartbeat);
        }
    }

    pub async fn is_authenticated(&self, id: Uuid) -> bool {
        if let Some(session) = self.sessions.get(&id) {
            return session.read().await.user().is_some();
        }
        false
    }

    pub async fn authenticate(&mut self, id: Uuid, user: String) {
        if let Some(session) = self.sessions.get(&id) {
            if let Some(user) = self.users.get_mut(&user) {
                user.set_session_id(id);
            }
            self.sync_access_level(id, &user).await;
            session.write().await.set_user(user);
        }
    }

    pub async fn get_access_level(&self, id: Uuid) -> AccessLevel {
        if let Some(session) = self.sessions.get(&id) {
            return session.read().await.access_level().clone();
        }
        AccessLevel::Guest
    }

    pub async fn sync_access_level(&self, id: Uuid, user: &str) {
        if let Some(session) = self.sessions.get(&id) {
            if let Some(user) = self.users.get(user) {
                session.write().await.set_access_level(user.access_level().clone());
            }
        }
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
