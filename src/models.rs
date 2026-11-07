use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct ModeRow {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub short_description: String,
    pub description: String,
    pub status: String,
    pub cover_url: Option<String>,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub size_label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub author_id: Uuid,
    pub author_username: String,
    pub author_name: String,
    pub author_avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSummary {
    pub id: Uuid,
    pub version: String,
    pub status: String,
    pub channel: String,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: Uuid,
    pub kind: String,
    pub url: String,
    pub size: i64,
    pub sha256: Option<String>,
    pub parts: Value,
    pub manifest: Option<Value>,
    pub file_name: Option<String>,
    pub storage_key: Option<String>,
    pub storage_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: Uuid,
    pub version: String,
    pub status: String,
    pub channel: String,
    pub changelog: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    #[sqlx(skip)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub files: u64,
    pub bytes: u64,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeSummary {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub short_description: String,
    pub description: String,
    pub status: String,
    pub cover_url: Option<String>,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub size: Option<String>,
    pub author: UserSummary,
    pub tags: Vec<Tag>,
    pub latest_release: Option<ReleaseSummary>,
    pub build: Option<BuildInfo>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogResponse {
    pub items: Vec<ModeSummary>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorResponse {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub modes: Vec<ModeSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModeRequest {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub short_description: String,
    #[serde(default)]
    pub description: String,
    pub cover_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub size: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReleaseRequest {
    pub version: String,
    pub channel: Option<String>,
    #[serde(default)]
    pub changelog: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadUrlRequest {
    pub kind: String,
    pub file_name: String,
    #[serde(default)]
    pub size: i64,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadUrlResponse {
    pub asset_id: Uuid,
    pub upload_url: String,
    pub method: &'static str,
    pub storage: String,
    pub complete_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteAssetRequest {
    #[serde(default)]
    pub size: Option<i64>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateModeResponse {
    pub id: Uuid,
    pub slug: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReleaseResponse {
    pub id: Uuid,
    pub version: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleStatus {
    pub ok: bool,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectRequest {
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ModerationItem {
    pub release_id: Uuid,
    pub version: String,
    pub mode_id: Uuid,
    pub mode_slug: String,
    pub mode_title: String,
    pub author_name: String,
    pub created_at: DateTime<Utc>,
}
