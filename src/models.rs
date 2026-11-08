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
    pub rejection_note: Option<String>,
    pub moderated_at: Option<DateTime<Utc>>,
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
    pub installs: i64,
    pub rating: Option<f64>,
    pub reviews_count: i64,
    pub screenshots: Vec<String>,
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
pub struct CollectionSection {
    pub id: String,
    pub title: String,
    pub modes: Vec<ModeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FollowedAuthor {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub followers_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    pub mode_slug: String,
    pub mode_title: String,
    pub version: String,
    pub changelog: String,
    pub published_at: Option<DateTime<Utc>>,
    pub cover_url: Option<String>,
    pub author_username: String,
    pub author_name: String,
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
    pub manifest: Option<Value>,
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
    pub author_username: String,
    pub changelog: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ModerationHistoryItem {
    pub id: Uuid,
    pub action: String,
    pub note: String,
    pub target_type: String,
    pub target_id: Uuid,
    pub moderator_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdminModeItem {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub status: String,
    pub author_name: String,
    pub author_username: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PublisherModeRow {
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PublisherStats {
    pub downloads: i64,
    pub installs: i64,
    pub updates: i64,
    pub unique_users: i64,
    pub last_event_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublisherMode {
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub releases: Vec<Release>,
    pub stats: PublisherStats,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeEventRequest {
    pub event: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverResponse {
    pub cover_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub id: Uuid,
    pub rating: i16,
    pub text: String,
    pub author_name: String,
    pub author_username: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewListResponse {
    pub average_rating: Option<f64>,
    pub count: i64,
    pub items: Vec<ReviewItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    pub rating: i16,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportRequest {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
    pub status: String,
    pub reporter_name: Option<String>,
    pub reporter_username: Option<String>,
    pub target_title: Option<String>,
    pub target_slug: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub avatar_url: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub minecraft_uuid: Option<String>,
    pub skin_url: Option<String>,
    pub skin_model: String,
    pub cape_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSessionResponse {
    pub access_token: String,
    pub client_token: String,
    pub expires_in: i64,
    pub profile: GameProfile,
    pub skin_url: Option<String>,
    pub skin_model: String,
    pub cape_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_expires_in: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterResponse {
    pub user: AccountUser,
    pub tokens: AuthTokens,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub user: AccountUser,
    pub tokens: AuthTokens,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgotPasswordResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct IdentitySummary {
    pub id: Uuid,
    pub provider: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}
