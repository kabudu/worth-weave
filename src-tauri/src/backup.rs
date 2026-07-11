use std::fs;
use std::io::{Read, Write};
use std::iter;
use std::path::Path;
use std::time::Duration;

use age::secrecy::SecretString;
use rusqlite::{Connection, backup::Backup};
use serde::Serialize;
use uuid::Uuid;

use crate::error::{LedgerlyError, Result};
use crate::{db, market, projections};

const MAX_BACKUP_BYTES: u64 = 1024 * 1024 * 1024;

fn validate(path: &Path, password: &str) -> Result<()> {
    if password.chars().count() < 12 {
        return Err(LedgerlyError::Backup(
            "password must contain at least 12 characters".into(),
        ));
    }
    if path.as_os_str().is_empty() {
        return Err(LedgerlyError::Backup("backup path is required".into()));
    }
    Ok(())
}

fn database_bytes(connection: &Connection) -> Result<Vec<u8>> {
    let file = tempfile::NamedTempFile::new()?;
    let mut destination = Connection::open(file.path())?;
    Backup::new(connection, &mut destination)?.run_to_completion(
        128,
        Duration::from_millis(5),
        None,
    )?;
    drop(destination);
    fs::read(file.path()).map_err(Into::into)
}

pub fn create(connection: &Connection, path: &Path, password: String) -> Result<()> {
    validate(path, &password)?;
    let plaintext = database_bytes(connection)?;
    let encryptor = age::Encryptor::with_user_passphrase(SecretString::from(password));
    let mut encrypted = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    writer.write_all(&plaintext)?;
    writer
        .finish()
        .map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    fs::write(&temporary, encrypted)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

pub fn restore(connection: &mut Connection, path: &Path, password: String) -> Result<()> {
    validate(path, &password)?;
    if fs::metadata(path)?.len() > MAX_BACKUP_BYTES {
        return Err(LedgerlyError::Backup(
            "backup exceeds the 1 GiB restore limit".into(),
        ));
    }
    let encrypted = fs::read(path)?;
    let decryptor = age::Decryptor::new(&encrypted[..])
        .map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    let identity = age::scrypt::Identity::new(SecretString::from(password));
    let reader = decryptor
        .decrypt(iter::once(&identity as _))
        .map_err(|_| LedgerlyError::Backup("password is incorrect or backup is damaged".into()))?;
    let mut plaintext = Vec::new();
    reader
        .take(MAX_BACKUP_BYTES + 1)
        .read_to_end(&mut plaintext)?;
    if plaintext.len() as u64 > MAX_BACKUP_BYTES {
        return Err(LedgerlyError::Backup(
            "decrypted backup exceeds the restore limit".into(),
        ));
    }
    let file = tempfile::NamedTempFile::new()?;
    fs::write(file.path(), plaintext)?;
    let source = Connection::open(file.path())?;
    let integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(LedgerlyError::Backup(
            "backup database failed integrity validation".into(),
        ));
    }
    source
        .query_row("SELECT id FROM app_settings WHERE id = 1", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|_| LedgerlyError::Backup("backup schema is not recognized".into()))?;
    Backup::new(&source, connection)?.run_to_completion(128, Duration::from_millis(5), None)?;
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}

#[derive(Serialize)]
struct PortfolioExport {
    format: &'static str,
    version: u32,
    exported_at: String,
    settings: crate::models::AppSettings,
    accounts: Vec<crate::models::Account>,
    holdings: Vec<crate::models::Holding>,
    income: Vec<crate::models::IncomeSummary>,
    valuation: crate::models::ValuationSummary,
    snapshots: Vec<crate::models::PortfolioSnapshot>,
}

pub fn export_json(connection: &Connection, path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(LedgerlyError::Backup("export path is required".into()));
    }
    let export = PortfolioExport {
        format: "worthweave-portfolio-export",
        version: 1,
        exported_at: chrono::Utc::now().to_rfc3339(),
        settings: db::settings(connection)?,
        accounts: db::accounts(connection)?,
        holdings: projections::holdings(connection)?,
        income: projections::income(connection)?,
        valuation: market::valuation(connection)?,
        snapshots: market::snapshots(connection)?,
    };
    let data = serde_json::to_vec_pretty(&export)
        .map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    let temporary = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    fs::write(&temporary, data)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
