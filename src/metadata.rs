use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: i64,
    pub world_time: f64,
    pub schema_version: u32,
    pub custom_fields: HashMap<String, String>,
    pub tags: Vec<String>,
}

impl SnapshotMetadata {
    pub fn new(id: String) -> Self {
        Self {
            id,
            name: None,
            description: None,
            created_at: chrono::Utc::now().timestamp(),
            world_time: 0.0,
            schema_version: 1,
            custom_fields: HashMap::new(),
            tags: Vec::new(),
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_tag(mut self, tag: String) -> Self {
        self.tags.push(tag);
        self
    }

    pub fn with_custom_field(mut self, key: String, value: String) -> Self {
        self.custom_fields.insert(key, value);
        self
    }
}
