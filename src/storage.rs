use crate::error::{PackError, Result};
use crate::format::{PackedSnapshot, SnapshotHeader, PackFormat};
use crate::compression::{CompressionCodec, compress, decompress};
use crate::metadata::SnapshotMetadata;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Write, Read};
use sha2::{Sha256, Digest};

#[cfg(feature = "encryption")]
use crate::encryption::{EncryptionKey, encrypt_snapshot, decrypt_snapshot};

pub struct SnapshotWriter {
    compression: CompressionCodec,
    #[cfg(feature = "encryption")]
    encryption_key: Option<EncryptionKey>,
}

impl SnapshotWriter {
    pub fn new() -> Self {
        Self {
            compression: CompressionCodec::zstd_default(),
            #[cfg(feature = "encryption")]
            encryption_key: None,
        }
    }

    pub fn with_compression(mut self, codec: CompressionCodec) -> Self {
        self.compression = codec;
        self
    }

    #[cfg(feature = "encryption")]
    pub fn with_encryption(mut self, key: EncryptionKey) -> Self {
        self.encryption_key = Some(key);
        self
    }

    pub fn write_to_file<P: AsRef<Path>>(
        &self,
        snapshot: &PackedSnapshot,
        path: P,
    ) -> Result<()> {
        let serialized = self.serialize_snapshot(snapshot)?;

        let compressed = compress(&serialized, self.compression)?;

        #[cfg(feature = "encryption")]
        let final_data = if let Some(key) = &self.encryption_key {
            encrypt_snapshot(&compressed, key)?
        } else {
            compressed
        };

        #[cfg(not(feature = "encryption"))]
        let final_data = compressed;

        let mut header = snapshot.header.clone();
        header.compression = self.compression.into();

        #[cfg(feature = "encryption")]
        {
            header.encrypted = self.encryption_key.is_some();
        }

        header.checksum = self.compute_checksum(&final_data);
        header.data_size = final_data.len() as u64;

        let header_bytes = bincode::serialize(&header)?;
        header.data_offset = header_bytes.len() as u64;

        let final_header_bytes = bincode::serialize(&header)?;

        let mut file = File::create(path)?;

        file.write_all(&final_header_bytes)?;

        file.write_all(&final_data)?;

        file.sync_all()?;

        Ok(())
    }

    pub fn write_to_bytes(&self, snapshot: &PackedSnapshot) -> Result<Vec<u8>> {
        let serialized = self.serialize_snapshot(snapshot)?;

        let compressed = compress(&serialized, self.compression)?;

        #[cfg(feature = "encryption")]
        let final_data = if let Some(key) = &self.encryption_key {
            encrypt_snapshot(&compressed, key)?
        } else {
            compressed
        };

        #[cfg(not(feature = "encryption"))]
        let final_data = compressed;

        let mut header = snapshot.header.clone();
        header.compression = self.compression.into();

        #[cfg(feature = "encryption")]
        {
            header.encrypted = self.encryption_key.is_some();
        }

        header.checksum = self.compute_checksum(&final_data);
        header.data_size = final_data.len() as u64;

        let header_bytes = bincode::serialize(&header)?;
        header.data_offset = header_bytes.len() as u64;

        let final_header_bytes = bincode::serialize(&header)?;

        let mut result = Vec::with_capacity(final_header_bytes.len() + final_data.len());
        result.extend_from_slice(&final_header_bytes);
        result.extend_from_slice(&final_data);

        Ok(result)
    }

    fn serialize_snapshot(&self, snapshot: &PackedSnapshot) -> Result<Vec<u8>> {
        match snapshot.header.format {
            PackFormat::Bincode => {
                bincode::serialize(snapshot)
                    .map_err(|e| PackError::Serialization(e.to_string()))
            }
            PackFormat::MessagePack => {
                rmp_serde::to_vec(snapshot)
                    .map_err(|e| PackError::Serialization(e.to_string()))
            }
            PackFormat::Custom => {
                Err(PackError::Serialization("Custom format not implemented".to_string()))
            }
        }
    }

    fn compute_checksum(&self, data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
}

impl Default for SnapshotWriter {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SnapshotReader {
    #[cfg(feature = "encryption")]
    encryption_key: Option<EncryptionKey>,
}

impl SnapshotReader {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "encryption")]
            encryption_key: None,
        }
    }

    #[cfg(feature = "encryption")]
    pub fn with_encryption(mut self, key: EncryptionKey) -> Self {
        self.encryption_key = Some(key);
        self
    }

    pub fn read_from_file<P: AsRef<Path>>(&self, path: P) -> Result<PackedSnapshot> {
        let mut file = File::open(path)?;

        let mut all_data = Vec::new();
        file.read_to_end(&mut all_data)?;

        let header: SnapshotHeader = bincode::deserialize(&all_data)?;
        header.validate()?;

        let data_start = header.data_offset as usize;
        let data_end = data_start + header.data_size as usize;

        if data_end > all_data.len() {
            return Err(PackError::InvalidFormat(
                format!("Data end {} exceeds file length {}", data_end, all_data.len())
            ));
        }

        let data = &all_data[data_start..data_end];

        self.verify_checksum(data, &header.checksum)?;

        let decompressed = if header.encrypted {
            #[cfg(feature = "encryption")]
            {
                let key = self.encryption_key.as_ref()
                    .ok_or_else(|| PackError::Decryption("No encryption key provided".to_string()))?;
                let decrypted = decrypt_snapshot(data, key)?;
                decompress(&decrypted, header.compression)?
            }

            #[cfg(not(feature = "encryption"))]
            {
                return Err(PackError::Decryption("Snapshot is encrypted but encryption feature is disabled".to_string()));
            }
        } else {
            decompress(data, header.compression)?
        };

        self.deserialize_snapshot(&decompressed, header.format)
    }

    pub fn read_from_bytes(&self, bytes: &[u8]) -> Result<PackedSnapshot> {
        let header: SnapshotHeader = bincode::deserialize(bytes)?;
        header.validate()?;

        let data_start = header.data_offset as usize;
        let data_end = data_start + header.data_size as usize;

        if data_end > bytes.len() {
            return Err(PackError::InvalidFormat(
                format!("Data end {} exceeds buffer length {}", data_end, bytes.len())
            ));
        }

        let data = &bytes[data_start..data_end];

        self.verify_checksum(data, &header.checksum)?;

        let decompressed = if header.encrypted {
            #[cfg(feature = "encryption")]
            {
                let key = self.encryption_key.as_ref()
                    .ok_or_else(|| PackError::Decryption("No encryption key provided".to_string()))?;
                let decrypted = decrypt_snapshot(data, key)?;
                decompress(&decrypted, header.compression)?
            }

            #[cfg(not(feature = "encryption"))]
            {
                return Err(PackError::Decryption("Snapshot is encrypted but encryption feature is disabled".to_string()));
            }
        } else {
            decompress(data, header.compression)?
        };

        self.deserialize_snapshot(&decompressed, header.format)
    }

    fn deserialize_snapshot(&self, data: &[u8], format: PackFormat) -> Result<PackedSnapshot> {
        match format {
            PackFormat::Bincode => {
                bincode::deserialize(data)
                    .map_err(|e| PackError::Deserialization(e.to_string()))
            }
            PackFormat::MessagePack => {
                rmp_serde::from_slice(data)
                    .map_err(|e| PackError::Deserialization(e.to_string()))
            }
            PackFormat::Custom => {
                Err(PackError::Deserialization("Custom format not implemented".to_string()))
            }
        }
    }

    fn verify_checksum(&self, data: &[u8], expected: &[u8; 32]) -> Result<()> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let actual: [u8; 32] = hasher.finalize().into();

        if &actual != expected {
            return Err(PackError::ChecksumMismatch);
        }

        Ok(())
    }
}

impl Default for SnapshotReader {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SnapshotStore {
    root_dir: PathBuf,
}

impl SnapshotStore {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&root_dir)?;

        Ok(Self { root_dir })
    }

    pub fn save(
        &self,
        snapshot: &PackedSnapshot,
        metadata: &SnapshotMetadata,
        writer: &SnapshotWriter,
    ) -> Result<PathBuf> {
        let filename = format!("{}.tx2pack", metadata.id);
        let path = self.root_dir.join(&filename);

        writer.write_to_file(snapshot, &path)?;

        let metadata_path = self.root_dir.join(format!("{}.meta.json", metadata.id));
        let metadata_json = serde_json::to_string_pretty(metadata)?;
        std::fs::write(metadata_path, metadata_json)?;

        Ok(path)
    }

    pub fn load(&self, id: &str, reader: &SnapshotReader) -> Result<(PackedSnapshot, SnapshotMetadata)> {
        let filename = format!("{}.tx2pack", id);
        let path = self.root_dir.join(&filename);

        if !path.exists() {
            return Err(PackError::SnapshotNotFound(id.to_string()));
        }

        let snapshot = reader.read_from_file(&path)?;

        let metadata_path = self.root_dir.join(format!("{}.meta.json", id));
        let metadata = if metadata_path.exists() {
            let metadata_json = std::fs::read_to_string(metadata_path)?;
            serde_json::from_str(&metadata_json)?
        } else {
            SnapshotMetadata::new(id.to_string())
        };

        Ok((snapshot, metadata))
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let filename = format!("{}.tx2pack", id);
        let path = self.root_dir.join(&filename);

        if path.exists() {
            std::fs::remove_file(path)?;
        }

        let metadata_path = self.root_dir.join(format!("{}.meta.json", id));
        if metadata_path.exists() {
            std::fs::remove_file(metadata_path)?;
        }

        Ok(())
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let mut snapshots = Vec::new();

        for entry in std::fs::read_dir(&self.root_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(ext) = path.extension() {
                if ext == "tx2pack" {
                    if let Some(stem) = path.file_stem() {
                        snapshots.push(stem.to_string_lossy().to_string());
                    }
                }
            }
        }

        Ok(snapshots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::PackedSnapshot;
    use tempfile::TempDir;

    #[test]
    fn test_write_read_snapshot() {
        let snapshot = PackedSnapshot::new();

        let writer = SnapshotWriter::new();
        let bytes = writer.write_to_bytes(&snapshot).unwrap();

        let reader = SnapshotReader::new();
        let loaded = reader.read_from_bytes(&bytes).unwrap();

        assert_eq!(snapshot.header.version, loaded.header.version);
    }

    #[test]
    fn test_snapshot_store() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path()).unwrap();

        let snapshot = PackedSnapshot::new();
        let metadata = SnapshotMetadata::new("test-snapshot".to_string());

        let writer = SnapshotWriter::new();
        store.save(&snapshot, &metadata, &writer).unwrap();

        let snapshots = store.list().unwrap();
        assert!(snapshots.contains(&"test-snapshot".to_string()));

        let reader = SnapshotReader::new();
        let (loaded, loaded_meta) = store.load("test-snapshot", &reader).unwrap();

        assert_eq!(snapshot.header.version, loaded.header.version);
        assert_eq!(metadata.id, loaded_meta.id);

        store.delete("test-snapshot").unwrap();
        let snapshots = store.list().unwrap();
        assert!(!snapshots.contains(&"test-snapshot".to_string()));
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn test_encrypted_snapshot() {
        use crate::encryption::EncryptionKey;

        let snapshot = PackedSnapshot::new();
        let key = EncryptionKey::generate();

        let writer = SnapshotWriter::new().with_encryption(key.clone());
        let bytes = writer.write_to_bytes(&snapshot).unwrap();

        let reader = SnapshotReader::new().with_encryption(key);
        let loaded = reader.read_from_bytes(&bytes).unwrap();

        assert_eq!(snapshot.header.version, loaded.header.version);
    }
}
