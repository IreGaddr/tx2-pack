use serde::{Deserialize, Serialize};
use tx2_link::{EntityId, ComponentId};
use ahash::AHashMap;
use std::collections::HashMap;

pub const MAGIC_NUMBER: &[u8; 8] = b"TX2PACK\0";
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackFormat {
    Bincode,
    MessagePack,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub format: PackFormat,
    pub compression: CompressionType,
    pub encrypted: bool,
    pub checksum: [u8; 32],
    pub timestamp: i64,
    pub entity_count: u64,
    pub component_count: u64,
    pub archetype_count: u64,
    pub data_offset: u64,
    pub data_size: u64,
    pub metadata_offset: u64,
    pub metadata_size: u64,
}

impl SnapshotHeader {
    pub fn new() -> Self {
        Self {
            magic: *MAGIC_NUMBER,
            version: FORMAT_VERSION,
            format: PackFormat::Bincode,
            compression: CompressionType::Zstd,
            encrypted: false,
            checksum: [0u8; 32],
            timestamp: chrono::Utc::now().timestamp(),
            entity_count: 0,
            component_count: 0,
            archetype_count: 0,
            data_offset: 0,
            data_size: 0,
            metadata_offset: 0,
            metadata_size: 0,
        }
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.magic != *MAGIC_NUMBER {
            return Err(crate::PackError::InvalidFormat(
                "Invalid magic number".to_string()
            ));
        }

        if self.version != FORMAT_VERSION {
            return Err(crate::PackError::VersionMismatch {
                expected: FORMAT_VERSION.to_string(),
                actual: self.version.to_string(),
            });
        }

        Ok(())
    }
}

impl Default for SnapshotHeader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Zstd,
    Lz4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentArchetype {
    pub component_id: ComponentId,
    pub entity_ids: Vec<EntityId>,
    pub data: ComponentData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentData {
    StructOfArrays(StructOfArraysData),
    Blob(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructOfArraysData {
    pub field_names: Vec<String>,
    pub field_types: Vec<FieldType>,
    pub field_data: Vec<FieldArray>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Bytes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldArray {
    Bool(Vec<bool>),
    I8(Vec<i8>),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    U8(Vec<u8>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
    String(Vec<String>),
    Bytes(Vec<Vec<u8>>),
}

impl FieldArray {
    pub fn len(&self) -> usize {
        match self {
            FieldArray::Bool(v) => v.len(),
            FieldArray::I8(v) => v.len(),
            FieldArray::I16(v) => v.len(),
            FieldArray::I32(v) => v.len(),
            FieldArray::I64(v) => v.len(),
            FieldArray::U8(v) => v.len(),
            FieldArray::U16(v) => v.len(),
            FieldArray::U32(v) => v.len(),
            FieldArray::U64(v) => v.len(),
            FieldArray::F32(v) => v.len(),
            FieldArray::F64(v) => v.len(),
            FieldArray::String(v) => v.len(),
            FieldArray::Bytes(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackedSnapshot {
    pub header: SnapshotHeader,
    pub archetypes: Vec<ComponentArchetype>,
    pub entity_metadata: HashMap<EntityId, EntityMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMetadata {
    pub created_at: i64,
    pub modified_at: i64,
    pub tags: Vec<String>,
}

impl PackedSnapshot {
    pub fn new() -> Self {
        Self {
            header: SnapshotHeader::new(),
            archetypes: Vec::new(),
            entity_metadata: HashMap::new(),
        }
    }

    pub fn from_world_snapshot(snapshot: tx2_link::WorldSnapshot) -> Self {
        let mut packed = Self::new();
        packed.header.timestamp = snapshot.timestamp as i64;

        let entity_count = snapshot.entities.len() as u64;

        let mut component_map: AHashMap<ComponentId, ComponentArchetype> = AHashMap::new();

        for entity in &snapshot.entities {
            for component in &entity.components {
                let archetype = component_map
                    .entry(component.id.clone())
                    .or_insert_with(|| ComponentArchetype {
                        component_id: component.id.clone(),
                        entity_ids: Vec::new(),
                        data: ComponentData::Blob(Vec::new()),
                    });

                archetype.entity_ids.push(entity.id);
            }
        }

        packed.archetypes = component_map.into_values().collect();
        packed.header.entity_count = entity_count;
        packed.header.component_count = packed.archetypes.len() as u64;
        packed.header.archetype_count = packed.archetypes.len() as u64;

        packed
    }
}

impl Default for PackedSnapshot {
    fn default() -> Self {
        Self::new()
    }
}
