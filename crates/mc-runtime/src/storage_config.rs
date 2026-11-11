use std::sync::Arc;

use mc_core::{CoreError, Result, StorageId};
use mc_crypto::MasterKey;
use mc_db::repos::storages::StorageRecord;
use mc_storage::{LocalStorage, S3Config, S3Storage, Storage};
use serde_json::{json, Value};

pub enum BackendConfig {
    Local { root: String },
    S3(S3Config),
}

pub fn encode_storage_config(
    kind: &str,
    config: &Value,
    master_key: Option<&MasterKey>,
) -> Result<Value> {
    match kind {
        "local" => {
            let root = required_str(config, "root")?;
            Ok(json!({ "root": root }))
        }
        "s3" => {
            let master_key = master_key.ok_or_else(|| {
                CoreError::Unavailable("MC_MASTER_KEY is required for S3 storage".to_owned())
            })?;
            let secret = required_str(config, "secret_access_key")?;
            let mut stored = config.clone();

            if let Value::Object(map) = &mut stored {
                map.remove("secret_access_key");
                map.insert(
                    "secret_access_key_enc".to_owned(),
                    Value::String(master_key.encrypt(secret.as_bytes())?),
                );
            }

            Ok(stored)
        }
        other => Err(CoreError::Validation(format!(
            "unsupported storage kind: {other}"
        ))),
    }
}

pub fn decode_storage_config(
    record: &StorageRecord,
    master_key: Option<&MasterKey>,
) -> Result<BackendConfig> {
    match record.kind.as_str() {
        "local" => Ok(BackendConfig::Local {
            root: required_str(&record.config, "root")?.to_owned(),
        }),
        "s3" => {
            let master_key = master_key.ok_or_else(|| {
                CoreError::Unavailable("MC_MASTER_KEY is required for S3 storage".to_owned())
            })?;
            let encrypted = required_str(&record.config, "secret_access_key_enc")?;
            let secret = String::from_utf8(master_key.decrypt(encrypted)?).map_err(|error| {
                CoreError::Validation(format!("S3 secret is not valid UTF-8: {error}"))
            })?;

            Ok(BackendConfig::S3(S3Config {
                endpoint: required_str(&record.config, "endpoint")?.to_owned(),
                region: optional_str(&record.config, "region", "us-east-1"),
                bucket: required_str(&record.config, "bucket")?.to_owned(),
                access_key_id: required_str(&record.config, "access_key_id")?.to_owned(),
                secret_access_key: secret,
                root: optional_str(&record.config, "root", ""),
                virtual_host_style: optional_bool(&record.config, "virtual_host_style", false),
                presign_seconds: optional_u64(&record.config, "presign_seconds", 3600),
            }))
        }
        other => Err(CoreError::Validation(format!(
            "unsupported storage kind: {other}"
        ))),
    }
}

pub fn build_storage(
    record: &StorageRecord,
    master_key: Option<&MasterKey>,
) -> Result<Arc<dyn Storage>> {
    let id = StorageId::from_uuid(record.id);

    match decode_storage_config(record, master_key)? {
        BackendConfig::Local { root } => Ok(Arc::new(LocalStorage::new(id, root)?)),
        BackendConfig::S3(config) => Ok(Arc::new(S3Storage::new(id, &config)?)),
    }
}

#[must_use]
pub fn redacted_config(record: &StorageRecord) -> Value {
    let mut config = record.config.clone();

    if let Value::Object(map) = &mut config {
        map.remove("secret_access_key");
        map.remove("secret_access_key_enc");
    }

    config
}

fn required_str<'a>(config: &'a Value, key: &str) -> Result<&'a str> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CoreError::Validation(format!("storage config requires {key}")))
}

fn optional_str(config: &Value, key: &str, default: &str) -> String {
    config
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn optional_bool(config: &Value, key: &str, default: bool) -> bool {
    config.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn optional_u64(config: &Value, key: &str, default: u64) -> u64 {
    config.get(key).and_then(Value::as_u64).unwrap_or(default)
}
