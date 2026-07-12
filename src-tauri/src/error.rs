use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum WorthweaveError {
    #[error("database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("CSV export is invalid: {0}")]
    Csv(String),
    #[error("account does not exist")]
    AccountNotFound,
    #[error("account type confirmation does not match the destination account")]
    AccountTypeMismatch,
    #[error("import exceeds the 50 MiB size limit")]
    ImportTooLarge,
    #[error("import contains too many rows")]
    ImportRowLimit,
    #[error("only CSV imports are accepted")]
    UnsupportedFile,
    #[error("Robinhood imports require a supported US CSV or UK statement export format")]
    UnsupportedBrokerImport,
    #[error("invalid account details: {0}")]
    InvalidAccount(String),
    #[error("invalid application settings: {0}")]
    InvalidSettings(String),
    #[error("invalid market data: {0}")]
    InvalidMarketData(String),
    #[error("local AI request failed: {0}")]
    LocalAi(String),
    #[error("backup operation failed: {0}")]
    Backup(String),
    #[error("application data directory is unavailable")]
    DataDirectoryUnavailable,
    #[error("application state is unavailable")]
    StateUnavailable,
}

impl Serialize for WorthweaveError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, WorthweaveError>;
