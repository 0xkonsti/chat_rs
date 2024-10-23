use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    name: String,
    pw_hash: String,
    session_id: Option<Uuid>,
}

impl User {
    pub fn new(name: &str, pw_hash: String) -> Self {
        Self {
            name: name.to_string(),
            pw_hash,
            session_id: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn pw_hash(&self) -> &str {
        &self.pw_hash
    }

    pub fn session_id(&self) -> Option<Uuid> {
        self.session_id
    }

    pub fn set_session_id(&mut self, session_id: Uuid) {
        self.session_id = Some(session_id);
    }

    pub fn remove_session_id(&mut self) {
        self.session_id = None;
    }
}
