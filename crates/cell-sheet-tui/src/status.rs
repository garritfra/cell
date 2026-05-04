#[derive(Debug, Default, Clone)]
pub struct StatusManager {
    message: Option<String>,
}

impl StatusManager {
    pub fn set(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    pub fn clear(&mut self) {
        self.message = None;
    }

    pub fn as_deref(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_clear_status_message() {
        let mut status = StatusManager::default();

        status.set("saved");
        assert_eq!(status.as_deref(), Some("saved"));

        status.clear();
        assert_eq!(status.as_deref(), None);
    }
}
