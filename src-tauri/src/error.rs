use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum LedgerlyError {
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
    #[error("this file has already been imported for the account")]
    DuplicateImport,
    #[error("import exceeds the 50 MiB size limit")]
    ImportTooLarge,
    #[error("only CSV imports are accepted")]
    UnsupportedFile,
    #[error("invalid account details: {0}")]
    InvalidAccount(String),
    #[error("invalid application settings: {0}")]
    InvalidSettings(String),
    #[error("invalid market data: {0}")]
    InvalidMarketData(String),
    #[error("backup operation failed: {0}")]
    Backup(String),
    #[error("application data directory is unavailable")]
    DataDirectoryUnavailable,
    #[error("application state is unavailable")]
    StateUnavailable,
}

impl Serialize for LedgerlyError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LedgerlyError>;
