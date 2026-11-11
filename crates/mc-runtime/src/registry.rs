use std::collections::HashMap;
use std::sync::Arc;

use mc_core::Result;
use mc_crypto::MasterKey;
use mc_db::repos::libraries::{self, LibraryRecord};
use mc_db::repos::storages;
use mc_db::PgPool;
use mc_storage::Storage;
use tracing::warn;
use uuid::Uuid;

use crate::storage_config::build_storage;

#[derive(Clone, Default)]
pub struct StorageRegistry {
    libraries: Arc<Vec<LibraryRecord>>,
    library_storages: Arc<HashMap<Uuid, Arc<dyn Storage>>>,
}

impl StorageRegistry {
    pub async fn load(pool: &PgPool, master_key: Option<&MasterKey>) -> Result<Self> {
        let storage_records = storages::list(pool).await?;
        let libraries = libraries::list(pool).await?;
        let mut built = HashMap::new();

        for record in &storage_records {
            match build_storage(record, master_key) {
                Ok(storage) => {
                    built.insert(record.id, storage);
                }
                Err(error) => {
                    warn!(storage = %record.name, %error, "failed to initialize storage");
                }
            }
        }

        let mut library_storages = HashMap::new();
        for library in &libraries {
            if let Some(storage) = built.get(&library.storage_id) {
                library_storages.insert(library.id, storage.clone());
            } else {
                warn!(library = %library.name, "library storage is unavailable");
            }
        }

        Ok(Self {
            libraries: Arc::new(libraries),
            library_storages: Arc::new(library_storages),
        })
    }

    #[must_use]
    pub fn libraries(&self) -> &[LibraryRecord] {
        &self.libraries
    }

    #[must_use]
    pub fn storage_for_library(&self, library_id: Uuid) -> Option<Arc<dyn Storage>> {
        self.library_storages.get(&library_id).cloned()
    }
}
