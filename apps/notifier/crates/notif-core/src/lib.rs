//! Shared types and traits for the `notif` CLI. Zero platform code.

#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub subtitle: Option<String>,
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct Sender {
    pub key: String,
}

pub trait Backend {
    type Error: std::error::Error;
    fn dispatch(&self, notif: &Notification) -> Result<(), Self::Error>;
}
