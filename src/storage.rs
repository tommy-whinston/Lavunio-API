use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use uuid::Uuid;

use crate::config::Config;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageKind {
    Local,
    S3,
}

impl StorageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::S3 => "s3",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalStorage {
    pub dir: PathBuf,
    pub public_base_url: String,
}

#[derive(Clone)]
pub struct S3Storage {
    pub client: Client,
    pub bucket: String,
    pub public_base_url: String,
}

#[derive(Clone)]
pub enum Storage {
    Local(LocalStorage),
    S3(S3Storage),
}

impl Storage {
    pub fn new(config: &Config) -> Result<Self, String> {
        match config.storage_kind {
            StorageKind::Local => {
                let _ = fs::create_dir_all(&config.storage_dir);
                Ok(Self::Local(LocalStorage {
                    dir: config.storage_dir.clone(),
                    public_base_url: config.public_base_url.clone(),
                }))
            }
            StorageKind::S3 => {
                let endpoint = config
                    .s3_endpoint
                    .as_deref()
                    .ok_or_else(|| "S3_ENDPOINT is required for S3 storage".to_string())?;
                let bucket = config
                    .s3_bucket
                    .as_deref()
                    .ok_or_else(|| "S3_BUCKET is required for S3 storage".to_string())?;
                let access_key = config
                    .s3_access_key_id
                    .as_deref()
                    .ok_or_else(|| "S3_ACCESS_KEY_ID is required for S3 storage".to_string())?;
                let secret_key = config
                    .s3_secret_access_key
                    .as_deref()
                    .ok_or_else(|| "S3_SECRET_ACCESS_KEY is required for S3 storage".to_string())?;

                let credentials =
                    Credentials::new(access_key, secret_key, None, None, "lavunio-api");
                let client = Client::from_conf(
                    aws_sdk_s3::config::Builder::new()
                        .behavior_version(BehaviorVersion::latest())
                        .region(Region::new(config.s3_region.clone()))
                        .endpoint_url(endpoint)
                        .credentials_provider(credentials)
                        .force_path_style(config.s3_force_path_style)
                        .build(),
                );

                let public_base_url = config.s3_public_base_url.clone().unwrap_or_else(|| {
                    format!(
                        "{}/{}",
                        endpoint.trim_end_matches('/'),
                        bucket.trim_matches('/')
                    )
                });

                Ok(Self::S3(S3Storage {
                    client,
                    bucket: bucket.to_string(),
                    public_base_url,
                }))
            }
        }
    }

    pub fn kind(&self) -> StorageKind {
        match self {
            Self::Local(_) => StorageKind::Local,
            Self::S3(_) => StorageKind::S3,
        }
    }

    pub fn local_root(&self) -> Option<&Path> {
        match self {
            Self::Local(storage) => Some(&storage.dir),
            Self::S3(_) => None,
        }
    }

    pub fn local_path(&self, key: &str) -> Option<PathBuf> {
        match self {
            Self::Local(storage) => Some(storage.dir.join(key)),
            Self::S3(_) => None,
        }
    }

    pub fn ensure_local_dir(&self) -> Result<(), String> {
        match self {
            Self::Local(storage) => fs::create_dir_all(&storage.dir)
                .map_err(|error| format!("Не удалось создать папку файлов: {error}")),
            Self::S3(_) => Ok(()),
        }
    }

    pub fn public_url(&self, key: &str) -> String {
        match self {
            Self::Local(storage) => format!(
                "{}/uploads/{}",
                storage.public_base_url.trim_end_matches('/'),
                key
            ),
            Self::S3(storage) => format!(
                "{}/{}",
                storage.public_base_url.trim_end_matches('/'),
                key.trim_start_matches('/')
            ),
        }
    }

    pub async fn presign_put(&self, key: &str, expires_secs: u64) -> Result<String, String> {
        let Self::S3(storage) = self else {
            return Err("local storage uses the API upload endpoint".to_string());
        };
        let presigning = PresigningConfig::expires_in(Duration::from_secs(expires_secs))
            .map_err(|error| format!("Не удалось настроить pre-signed URL: {error}"))?;
        let request = storage
            .client
            .put_object()
            .bucket(&storage.bucket)
            .key(key)
            .presigned(presigning)
            .await
            .map_err(|error| format!("Не удалось создать pre-signed URL: {error}"))?;
        Ok(request.uri().to_string())
    }

    pub async fn head_size(&self, key: &str) -> Result<u64, String> {
        let Self::S3(storage) = self else {
            return Err("head_size is only available for S3 storage".to_string());
        };
        let output = storage
            .client
            .head_object()
            .bucket(&storage.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| format!("Не удалось проверить файл в S3: {error}"))?;
        Ok(output.content_length().unwrap_or(0).max(0) as u64)
    }

    pub async fn get_bytes(&self, key: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Local(storage) => {
                let path = storage.dir.join(key);
                tokio::fs::read(&path)
                    .await
                    .map_err(|error| format!("Не удалось прочитать {}: {error}", path.display()))
            }
            Self::S3(storage) => {
                let output = storage
                    .client
                    .get_object()
                    .bucket(&storage.bucket)
                    .key(key)
                    .send()
                    .await
                    .map_err(|error| format!("Не удалось скачать файл из S3: {error}"))?;
                let data = output
                    .body
                    .collect()
                    .await
                    .map_err(|error| format!("Не удалось прочитать файл из S3: {error}"))?;
                Ok(data.into_bytes().to_vec())
            }
        }
    }

    pub async fn put_bytes(
        &self,
        key: &str,
        bytes: &[u8],
        content_type: Option<&str>,
    ) -> Result<(), String> {
        match self {
            Self::Local(storage) => {
                let path = storage.dir.join(key);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("Не удалось создать папку файлов: {error}"))?;
                }
                fs::write(path, bytes)
                    .map_err(|error| format!("Не удалось сохранить файл: {error}"))
            }
            Self::S3(storage) => {
                let mut request = storage
                    .client
                    .put_object()
                    .bucket(&storage.bucket)
                    .key(key)
                    .body(ByteStream::from(bytes.to_vec()));
                if let Some(content_type) = content_type {
                    request = request.content_type(content_type);
                }
                request
                    .send()
                    .await
                    .map_err(|error| format!("Не удалось загрузить файл в S3: {error}"))?;
                Ok(())
            }
        }
    }

    pub fn asset_key(mode_id: Uuid, release_id: Uuid, asset_id: Uuid, file_name: &str) -> String {
        format!(
            "modes/{mode_id}/releases/{release_id}/{asset_id}-{}",
            safe_file_name(file_name)
        )
    }
}

fn safe_file_name(value: &str) -> String {
    let name = value.rsplit(['/', '\\']).next().unwrap_or(value).trim();
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|ch| ch == '.' || ch == '_' || ch == '-');
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned.chars().take(120).collect()
    }
}
