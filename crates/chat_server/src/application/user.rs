use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct User {
    name: String,
    session_id: Option<Uuid>,
}

impl User {
    pub fn new(name: String) -> Self {
        Self {
            name,
            session_id: None,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_session_id(&mut self, session_id: Uuid) {
        self.session_id = Some(session_id);
    }

    pub fn remove_session_id(&mut self) {
        self.session_id = None;
    }
}
