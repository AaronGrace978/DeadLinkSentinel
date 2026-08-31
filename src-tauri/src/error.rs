use serde::Serialize;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Message(String),
}

impl AppError {
    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<url::ParseError> for AppError {
    fn from(e: url::ParseError) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<lettre::error::Error> for AppError {
    fn from(e: lettre::error::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<lettre::transport::smtp::Error> for AppError {
    fn from(e: lettre::transport::smtp::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<lettre::address::AddressError> for AppError {
    fn from(e: lettre::address::AddressError) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        Self::Message(e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        Self::Message(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
