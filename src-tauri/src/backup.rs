use std::fs;
use std::io::{Read, Write};
use std::iter;
use std::path::Path;
use std::time::Duration;

use age::secrecy::SecretString;
use rusqlite::{Connection, OptionalExtension, backup::Backup};
use serde::Serialize;

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
    let valid_extension = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("age") || value.eq_ignore_ascii_case("worthweave-age")
        });
    if !valid_extension {
        return Err(LedgerlyError::Backup(
            "encrypted backups must use the .age extension".into(),
        ));
    }
    Ok(())
}

fn database_snapshot(connection: &Connection) -> Result<tempfile::NamedTempFile> {
    let file = tempfile::NamedTempFile::new()?;
    let mut destination = Connection::open(file.path())?;
    Backup::new(connection, &mut destination)?.run_to_completion(
        128,
        Duration::from_millis(5),
        None,
    )?;
    drop(destination);
    Ok(file)
}

fn destination_temp(path: &Path) -> Result<tempfile::NamedTempFile> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    tempfile::Builder::new()
        .prefix(".worthweave-")
        .tempfile_in(parent)
        .map_err(Into::into)
}

fn persist(temp: tempfile::NamedTempFile, path: &Path) -> Result<()> {
    temp.persist(path)
        .map(|_| ())
        .map_err(|error| LedgerlyError::Io(error.error))
}

pub fn create(connection: &Connection, path: &Path, password: String) -> Result<()> {
    validate(path, &password)?;
    let plaintext = database_snapshot(connection)?;
    let encryptor = age::Encryptor::with_user_passphrase(SecretString::from(password));
    let mut encrypted = destination_temp(path)?;
    let mut writer = encryptor
        .wrap_output(encrypted.as_file_mut())
        .map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    let mut source = fs::File::open(plaintext.path())?;
    std::io::copy(&mut source, &mut writer)?;
    writer
        .finish()
        .map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    encrypted.as_file().sync_all()?;
    persist(encrypted, path)
}

pub fn restore(connection: &mut Connection, path: &Path, password: String) -> Result<()> {
    validate(path, &password)?;
    if fs::metadata(path)?.len() > MAX_BACKUP_BYTES {
        return Err(LedgerlyError::Backup(
            "backup exceeds the 1 GiB restore limit".into(),
        ));
    }
    let encrypted = fs::File::open(path)?;
    let decryptor =
        age::Decryptor::new(encrypted).map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    let identity = age::scrypt::Identity::new(SecretString::from(password));
    let reader = decryptor
        .decrypt(iter::once(&identity as _))
        .map_err(|_| LedgerlyError::Backup("password is incorrect or backup is damaged".into()))?;
    let mut file = tempfile::NamedTempFile::new()?;
    let copied = std::io::copy(&mut reader.take(MAX_BACKUP_BYTES + 1), file.as_file_mut())?;
    if copied > MAX_BACKUP_BYTES {
        return Err(LedgerlyError::Backup(
            "decrypted backup exceeds the restore limit".into(),
        ));
    }
    file.as_file_mut().flush()?;
    let source = Connection::open(file.path())?;
    let integrity: String = source.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(LedgerlyError::Backup(
            "backup database failed integrity validation".into(),
        ));
    }
    let version: i64 = source.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if !(1..=db::SCHEMA_VERSION).contains(&version) {
        return Err(LedgerlyError::Backup(
            "backup schema version is not supported by this Worthweave release".into(),
        ));
    }
    let foreign_key_violation: Option<i64> = source
        .query_row(
            "SELECT 1 FROM pragma_foreign_key_check LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if foreign_key_violation.is_some() {
        return Err(LedgerlyError::Backup(
            "backup contains invalid account relationships".into(),
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
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("json"))
    {
        return Err(LedgerlyError::Backup(
            "portfolio exports must use the .json extension".into(),
        ));
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
    let mut temporary = destination_temp(path)?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), &export)
        .map_err(|error| LedgerlyError::Backup(error.to_string()))?;
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;
    persist(temporary, path)
}
