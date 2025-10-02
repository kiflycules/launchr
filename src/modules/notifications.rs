use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub struct NotificationItem {
    pub title: String,
    pub message: String,
    pub level: String,
    pub timestamp: DateTime<Local>,
}

pub struct NotificationsModule {
    pub notifications: Vec<NotificationItem>,
}

impl NotificationsModule {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
        }
    }

    pub fn push(&mut self, title: impl Into<String>, message: impl Into<String>, level: &str) {
        self.notifications.insert(
            0,
            NotificationItem {
                title: title.into(),
                message: message.into(),
                level: level.to_string(),
                timestamp: Local::now(),
            },
        );
        if self.notifications.len() > 100 {
            self.notifications.pop();
        }
    }
}
