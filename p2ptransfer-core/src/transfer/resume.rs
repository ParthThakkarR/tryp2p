use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum TransferStatus {
    Pending,
    InProgress,
    Paused,
    Completed,
    Failed,
}

impl TransferStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_status_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferRecord {
    pub id: String,
    pub peer_addr: String,
    pub file_path: String,
    pub file_size: i64,
    pub bytes_transferred: i64,
    pub checksum: Option<String>,
    pub status: TransferStatus,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct Contact {
    pub name: String,
    pub peer_id: String,
    pub last_known_ip: String,
    pub last_known_port: u16,
    pub last_seen: i64,
}

pub struct TransferResumeManager {
    conn: Mutex<Connection>,
    _data_dir: PathBuf,
}

impl TransferResumeManager {
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&data_dir).context("Failed to create resume data directory")?;
        let db_path = data_dir.join("transfers.db");
        let conn = Connection::open(&db_path).context("Failed to open transfer resume database")?;

        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS transfers (
                id TEXT PRIMARY KEY,
                peer_addr TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                bytes_transferred INTEGER DEFAULT 0,
                checksum TEXT,
                status TEXT CHECK(status IN ('pending', 'in_progress', 'paused', 'completed', 'failed')) NOT NULL DEFAULT 'pending',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
             CREATE TABLE IF NOT EXISTS contacts (
                name TEXT PRIMARY KEY,
                peer_id TEXT NOT NULL,
                last_known_ip TEXT NOT NULL,
                last_known_port INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );",
        )
        .context("Failed to create transfers table")?;

        Ok(Self {
            conn: Mutex::new(conn),
            _data_dir: data_dir,
        })
    }

    pub fn create_transfer(
        &self,
        peer_addr: &str,
        file_path: &str,
        file_size: i64,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO transfers (id, peer_addr, file_path, file_size, bytes_transferred, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, 'pending', ?5, ?5)",
            rusqlite::params![id, peer_addr, file_path, file_size, now],
        )
        .context("Failed to insert transfer record")?;

        Ok(id)
    }

    // --- Contacts Management ---

    pub fn upsert_contact(
        &self,
        name: &str,
        peer_id: &str,
        ip: &str,
        port: u16,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO contacts (name, peer_id, last_known_ip, last_known_port, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(name) DO UPDATE SET
                peer_id = excluded.peer_id,
                last_known_ip = excluded.last_known_ip,
                last_known_port = excluded.last_known_port,
                last_seen = excluded.last_seen",
            rusqlite::params![name, peer_id, ip, port, now],
        )?;
        Ok(())
    }

    pub fn get_contact(&self, name: &str) -> Result<Option<Contact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name, peer_id, last_known_ip, last_known_port, last_seen FROM contacts WHERE name = ?1")?;
        let mut rows = stmt.query(rusqlite::params![name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Contact {
                name: row.get(0)?,
                peer_id: row.get(1)?,
                last_known_ip: row.get(2)?,
                last_known_port: row.get(3)?,
                last_seen: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_contacts(&self) -> Result<Vec<Contact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name, peer_id, last_known_ip, last_known_port, last_seen FROM contacts ORDER BY name ASC")?;
        let contact_iter = stmt.query_map([], |row| {
            Ok(Contact {
                name: row.get(0)?,
                peer_id: row.get(1)?,
                last_known_ip: row.get(2)?,
                last_known_port: row.get(3)?,
                last_seen: row.get(4)?,
            })
        })?;
        let mut contacts = Vec::new();
        for contact in contact_iter {
            contacts.push(contact?);
        }
        Ok(contacts)
    }

    pub fn delete_contact(&self, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM contacts WHERE name = ?1", rusqlite::params![name])?;
        Ok(())
    }

    pub fn update_transfer_status(&self, id: &str, status: TransferStatus) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE transfers SET status = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![status.as_str(), now, id],
        )
        .context("Failed to update transfer status")?;

        Ok(())
    }

    pub fn update_progress(&self, id: &str, bytes_transferred: i64) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE transfers SET bytes_transferred = ?1, status = 'in_progress', updated_at = ?2 WHERE id = ?3",
            rusqlite::params![bytes_transferred, now, id],
        )
        .context("Failed to update transfer progress")?;

        Ok(())
    }

    pub fn pause_transfer(&self, id: &str, bytes_transferred: i64) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE transfers SET bytes_transferred = ?1, status = 'paused', updated_at = ?2 WHERE id = ?3",
            rusqlite::params![bytes_transferred, now, id],
        )
        .context("Failed to pause transfer")?;

        Ok(())
    }

    pub fn complete_transfer(&self, id: &str, checksum: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.conn.lock().unwrap();
        let file_size: i64 = conn
            .query_row(
                "SELECT file_size FROM transfers WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        conn.execute(
            "UPDATE transfers SET bytes_transferred = ?1, checksum = ?2, status = 'completed', updated_at = ?3 WHERE id = ?4",
            rusqlite::params![file_size, checksum, now, id],
        )
        .context("Failed to complete transfer")?;

        Ok(())
    }

    pub fn fail_transfer(&self, id: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE transfers SET status = 'failed', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )
        .context("Failed to mark transfer as failed")?;

        Ok(())
    }

    pub fn get_transfer(&self, id: &str) -> Result<Option<TransferRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, peer_addr, file_path, file_size, bytes_transferred, checksum, status, created_at, updated_at
                 FROM transfers WHERE id = ?1",
            )
            .context("Failed to prepare query")?;

        let mut rows = stmt
            .query_map(rusqlite::params![id], |row| {
                let status_str: String = row.get(6)?;
                Ok(TransferRecord {
                    id: row.get(0)?,
                    peer_addr: row.get(1)?,
                    file_path: row.get(2)?,
                    file_size: row.get(3)?,
                    bytes_transferred: row.get(4)?,
                    checksum: row.get(5)?,
                    status: TransferStatus::from_status_str(&status_str)
                        .unwrap_or(TransferStatus::Failed),
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .context("Failed to query transfer")?;

        match rows.next() {
            Some(Ok(record)) => Ok(Some(record)),
            Some(Err(e)) => Err(anyhow::anyhow!("Failed to read transfer record: {e}")),
            None => Ok(None),
        }
    }

    pub fn list_transfers(&self) -> Result<Vec<TransferRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, peer_addr, file_path, file_size, bytes_transferred, checksum, status, created_at, updated_at
                 FROM transfers ORDER BY updated_at DESC",
            )
            .context("Failed to prepare list query")?;

        let rows = stmt
            .query_map([], |row| {
                let status_str: String = row.get(6)?;
                Ok(TransferRecord {
                    id: row.get(0)?,
                    peer_addr: row.get(1)?,
                    file_path: row.get(2)?,
                    file_size: row.get(3)?,
                    bytes_transferred: row.get(4)?,
                    checksum: row.get(5)?,
                    status: TransferStatus::from_status_str(&status_str)
                        .unwrap_or(TransferStatus::Failed),
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .context("Failed to query transfers")?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.context("Failed to read transfer row")?);
        }
        Ok(records)
    }

    pub fn delete_transfer(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM transfers WHERE id = ?1", rusqlite::params![id])
            .context("Failed to delete transfer")?;
        Ok(())
    }

    pub fn db_path(&self) -> PathBuf {
        self._data_dir.join("transfers.db")
    }

    pub fn cleanup_old_transfers(&self, retention_days: u64) -> Result<usize> {
        let cutoff = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            - (retention_days * 86400) as i64;

        let conn = self.conn.lock().unwrap();
        let deleted = conn
            .execute(
                "DELETE FROM transfers WHERE updated_at < ?1 AND status IN ('completed', 'failed')",
                rusqlite::params![cutoff],
            )
            .context("Failed to cleanup old transfers")?;

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_transfer() {
        let dir = tempfile::tempdir().unwrap();
        let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();

        let id = manager
            .create_transfer("192.168.1.10:9877", "/tmp/test.txt", 1024)
            .unwrap();

        let record = manager
            .get_transfer(&id)
            .unwrap()
            .expect("Transfer should exist");
        assert_eq!(record.peer_addr, "192.168.1.10:9877");
        assert_eq!(record.file_size, 1024);
        assert_eq!(record.bytes_transferred, 0);
        assert!(matches!(record.status, TransferStatus::Pending));
    }

    #[test]
    fn test_update_progress() {
        let dir = tempfile::tempdir().unwrap();
        let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();

        let id = manager
            .create_transfer("10.0.0.1:9877", "/tmp/large.bin", 10000)
            .unwrap();

        manager.update_progress(&id, 5000).unwrap();
        let record = manager.get_transfer(&id).unwrap().unwrap();
        assert_eq!(record.bytes_transferred, 5000);
        assert!(matches!(record.status, TransferStatus::InProgress));
    }

    #[test]
    fn test_pause_and_resume() {
        let dir = tempfile::tempdir().unwrap();
        let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();

        let id = manager
            .create_transfer("10.0.0.2:9877", "/tmp/resume.dat", 50000)
            .unwrap();

        manager.pause_transfer(&id, 25000).unwrap();
        let record = manager.get_transfer(&id).unwrap().unwrap();
        assert_eq!(record.bytes_transferred, 25000);
        assert!(matches!(record.status, TransferStatus::Paused));
    }

    #[test]
    fn test_complete_transfer() {
        let dir = tempfile::tempdir().unwrap();
        let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();

        let id = manager
            .create_transfer("10.0.0.3:9877", "/tmp/complete.dat", 100)
            .unwrap();

        manager.complete_transfer(&id, "abcdef1234567890").unwrap();
        let record = manager.get_transfer(&id).unwrap().unwrap();
        assert!(matches!(record.status, TransferStatus::Completed));
        assert_eq!(record.checksum.as_deref(), Some("abcdef1234567890"));
        assert_eq!(record.bytes_transferred, 100);
    }

    #[test]
    fn test_list_transfers() {
        let dir = tempfile::tempdir().unwrap();
        let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();

        manager
            .create_transfer("10.0.0.1:9877", "/tmp/a.txt", 100)
            .unwrap();
        manager
            .create_transfer("10.0.0.2:9877", "/tmp/b.txt", 200)
            .unwrap();

        let list = manager.list_transfers().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_get_nonexistent_transfer() {
        let dir = tempfile::tempdir().unwrap();
        let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();

        let record = manager.get_transfer("nonexistent-id").unwrap();
        assert!(record.is_none());
    }

    #[test]
    fn test_delete_transfer() {
        let dir = tempfile::tempdir().unwrap();
        let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();

        let id = manager
            .create_transfer("10.0.0.1:9877", "/tmp/delete_me.txt", 50)
            .unwrap();

        manager.delete_transfer(&id).unwrap();
        let record = manager.get_transfer(&id).unwrap();
        assert!(record.is_none());
    }

    #[test]
    fn test_transfer_status_as_str() {
        assert_eq!(TransferStatus::Pending.as_str(), "pending");
        assert_eq!(TransferStatus::InProgress.as_str(), "in_progress");
        assert_eq!(TransferStatus::Paused.as_str(), "paused");
        assert_eq!(TransferStatus::Completed.as_str(), "completed");
        assert_eq!(TransferStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn test_crash_persistence_bytes_transferred_survives_drop() {
        let dir = tempfile::tempdir().unwrap();
        let _db_path = dir.path().join("transfers.db");

        let offset: i64;
        {
            let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();
            let id = manager
                .create_transfer("192.168.1.50:9877", "/crash_test/file.bin", 50000)
                .unwrap();
            manager.update_progress(&id, 12345).unwrap();
            offset = manager
                .get_transfer(&id)
                .unwrap()
                .unwrap()
                .bytes_transferred;
            // manager drops here — simulates kill -9
        }

        // New process, same DB file
        {
            let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();
            let records = manager.list_transfers().unwrap();
            assert_eq!(records.len(), 1, "Record must survive after manager drop");
            assert_eq!(
                records[0].bytes_transferred, offset,
                "bytes_transferred must persist across crash: expected {offset}, got {}",
                records[0].bytes_transferred
            );
            assert!(matches!(records[0].status, TransferStatus::InProgress));
        }
    }

    #[test]
    fn test_crash_persistence_multiple_updates() {
        let dir = tempfile::tempdir().unwrap();

        let id;
        let expected: i64;
        {
            let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();
            id = manager
                .create_transfer("10.0.0.1:9877", "/multi_crash/data.bin", 100000)
                .unwrap();
            manager.update_progress(&id, 10000).unwrap();
            manager.update_progress(&id, 25000).unwrap();
            manager.update_progress(&id, 42000).unwrap();
            expected = 42000;
            // drop simulates crash
        }

        {
            let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();
            let record = manager.get_transfer(&id).unwrap().unwrap();
            assert_eq!(
                record.bytes_transferred, expected,
                "Last update ({expected}) must persist, got {}",
                record.bytes_transferred
            );
            assert_eq!(record.file_size, 100000);
        }
    }

    #[test]
    fn test_crash_persistence_status_paused_survives() {
        let dir = tempfile::tempdir().unwrap();

        let id;
        {
            let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();
            id = manager
                .create_transfer("10.0.0.2:9877", "/pause_crash/resume.dat", 75000)
                .unwrap();
            manager.update_progress(&id, 30000).unwrap();
            manager.pause_transfer(&id, 30000).unwrap();
            // drop simulates crash
        }

        {
            let manager = TransferResumeManager::new(dir.path().to_path_buf()).unwrap();
            let record = manager.get_transfer(&id).unwrap().unwrap();
            assert!(matches!(record.status, TransferStatus::Paused));
            assert_eq!(record.bytes_transferred, 30000);
        }
    }
}
