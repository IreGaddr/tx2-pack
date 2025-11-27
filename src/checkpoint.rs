use crate::error::Result;
use crate::format::PackedSnapshot;
use crate::metadata::SnapshotMetadata;
use crate::storage::{SnapshotWriter, SnapshotReader, SnapshotStore};
use std::path::Path;
use ahash::AHashMap;

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub id: String,
    pub snapshot: PackedSnapshot,
    pub metadata: SnapshotMetadata,
    pub parent_id: Option<String>,
}

impl Checkpoint {
    pub fn new(id: String, snapshot: PackedSnapshot) -> Self {
        let metadata = SnapshotMetadata::new(id.clone());

        Self {
            id: id.clone(),
            snapshot,
            metadata,
            parent_id: None,
        }
    }

    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn with_metadata(mut self, metadata: SnapshotMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

pub struct CheckpointManager {
    store: SnapshotStore,
    writer: SnapshotWriter,
    reader: SnapshotReader,
    checkpoints: AHashMap<String, Checkpoint>,
    checkpoint_chain: Vec<String>,
}

impl CheckpointManager {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        let store = SnapshotStore::new(root_dir)?;
        let writer = SnapshotWriter::new();
        let reader = SnapshotReader::new();

        Ok(Self {
            store,
            writer,
            reader,
            checkpoints: AHashMap::new(),
            checkpoint_chain: Vec::new(),
        })
    }

    pub fn with_writer(mut self, writer: SnapshotWriter) -> Self {
        self.writer = writer;
        self
    }

    pub fn with_reader(mut self, reader: SnapshotReader) -> Self {
        self.reader = reader;
        self
    }

    pub fn create_checkpoint(
        &mut self,
        id: String,
        snapshot: PackedSnapshot,
    ) -> Result<()> {
        let parent_id = self.checkpoint_chain.last().cloned();

        let mut checkpoint = Checkpoint::new(id.clone(), snapshot);
        if let Some(parent) = parent_id {
            checkpoint = checkpoint.with_parent(parent);
        }

        self.store.save(&checkpoint.snapshot, &checkpoint.metadata, &self.writer)?;

        self.checkpoint_chain.push(id.clone());
        self.checkpoints.insert(id, checkpoint);

        Ok(())
    }

    pub fn load_checkpoint(&mut self, id: &str) -> Result<Checkpoint> {
        if let Some(checkpoint) = self.checkpoints.get(id) {
            return Ok(checkpoint.clone());
        }

        let (snapshot, metadata) = self.store.load(id, &self.reader)?;

        let checkpoint = Checkpoint {
            id: id.to_string(),
            snapshot,
            metadata,
            parent_id: None,
        };

        self.checkpoints.insert(id.to_string(), checkpoint.clone());

        Ok(checkpoint)
    }

    pub fn delete_checkpoint(&mut self, id: &str) -> Result<()> {
        self.store.delete(id)?;
        self.checkpoints.remove(id);
        self.checkpoint_chain.retain(|cid| cid != id);
        Ok(())
    }

    pub fn list_checkpoints(&self) -> Result<Vec<String>> {
        self.store.list()
    }

    pub fn get_checkpoint_chain(&self) -> &[String] {
        &self.checkpoint_chain
    }

    pub fn get_latest_checkpoint(&self) -> Option<&str> {
        self.checkpoint_chain.last().map(|s| s.as_str())
    }

    pub fn prune_old_checkpoints(&mut self, keep_count: usize) -> Result<()> {
        let chain_len = self.checkpoint_chain.len();

        if chain_len <= keep_count {
            return Ok(());
        }

        let to_remove = chain_len - keep_count;

        for _ in 0..to_remove {
            if let Some(id) = self.checkpoint_chain.first().cloned() {
                self.delete_checkpoint(&id)?;
            }
        }

        Ok(())
    }

    pub fn clear_all_checkpoints(&mut self) -> Result<()> {
        for id in self.checkpoint_chain.clone() {
            self.delete_checkpoint(&id)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_checkpoint_manager() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = CheckpointManager::new(temp_dir.path()).unwrap();

        let snapshot1 = PackedSnapshot::new();
        manager.create_checkpoint("cp1".to_string(), snapshot1).unwrap();

        let snapshot2 = PackedSnapshot::new();
        manager.create_checkpoint("cp2".to_string(), snapshot2).unwrap();

        assert_eq!(manager.get_checkpoint_chain().len(), 2);
        assert_eq!(manager.get_latest_checkpoint(), Some("cp2"));

        let loaded = manager.load_checkpoint("cp1").unwrap();
        assert_eq!(loaded.id, "cp1");

        manager.prune_old_checkpoints(1).unwrap();
        assert_eq!(manager.get_checkpoint_chain().len(), 1);
        assert_eq!(manager.get_latest_checkpoint(), Some("cp2"));
    }

    #[test]
    fn test_checkpoint_clear() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = CheckpointManager::new(temp_dir.path()).unwrap();

        for i in 0..5 {
            let snapshot = PackedSnapshot::new();
            manager.create_checkpoint(format!("cp{}", i), snapshot).unwrap();
        }

        assert_eq!(manager.get_checkpoint_chain().len(), 5);

        manager.clear_all_checkpoints().unwrap();
        assert_eq!(manager.get_checkpoint_chain().len(), 0);
    }
}
