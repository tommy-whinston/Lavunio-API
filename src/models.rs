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
    pub trailer_url: Option<String>,
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
    pub team_id: Option<Uuid>,
    pub team_slug: Option<String>,
    pub team_title: Option<String>,
    pub team_verified: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamRef {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub verified: bool,
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
    #[sqlx(default)]
    pub appeal_status: Option<String>,
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
    pub trailer_url: Option<String>,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub size: Option<String>,
    pub installs: i64,
    pub rating: Option<f64>,
    pub reviews_count: i64,
    pub screenshots: Vec<String>,
    pub author: UserSummary,
    pub team: Option<TeamRef>,
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
pub struct AdminCollectionItem {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub sort_order: i32,
    pub mode_slugs: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionRequest {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub mode_slugs: Vec<String>,
    pub sort_order: Option<i32>,
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: Uuid,
    pub mode_id: Option<Uuid>,
    pub mode_slug: Option<String>,
    pub mode_title: Option<String>,
    pub title: String,
    pub body: String,
    pub author_name: String,
    pub author_username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsRequest {
    pub title: String,
    pub body: String,
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
    pub trailer_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub size: Option<String>,
    pub team_id: Option<Uuid>,
    #[serde(default)]
    pub co_authors: Vec<String>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppealRequest {
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveAppealRequest {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub links: String,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub owner_id: Uuid,
    pub status: String,
    pub join_mode: String,
    pub verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TeamCard {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub links: String,
    pub status: String,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub role: String,
    pub verified: bool,
    pub join_mode: String,
    pub modes_count: i64,
    pub downloads: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TeamModeItem {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub short_description: String,
    pub status: String,
    pub cover_url: Option<String>,
    pub installs: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TeamStats {
    pub modes: i64,
    pub published: i64,
    pub downloads: i64,
    pub installs: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TeamNewsItem {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TeamJoinRequestItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamProfile {
    pub team: Team,
    pub members: Vec<TeamMember>,
    pub modes: Vec<TeamModeItem>,
    pub stats: TeamStats,
    pub followers: i64,
    pub news: Vec<TeamNewsItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamRequest {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub links: String,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    #[serde(default)]
    pub join_mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamJoinRequest {
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamJoinResolveRequest {
    #[serde(default)]
    pub action: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamNewsRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamTransferRequest {
    #[serde(default)]
    pub username: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamVerifyRequest {
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TeamMember {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTeamItem {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub role: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlayerModeItem {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub short_description: String,
    pub cover_url: Option<String>,
    pub status: String,
    pub installs: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub modes: i64,
    pub downloads: i64,
    pub installs: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerProfile {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub bio: String,
    pub skin_url: Option<String>,
    pub skin_model: String,
    pub cape_url: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub online: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub teams: Vec<PlayerTeamItem>,
    pub modes: Vec<PlayerModeItem>,
    pub stats: PlayerStats,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub bio: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FriendItem {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub skin_url: Option<String>,
    pub online: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequestItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequests {
    pub incoming: Vec<FriendRequestItem>,
    pub outgoing: Vec<FriendRequestItem>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub link_type: Option<String>,
    pub link_id: Option<String>,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsResponse {
    pub items: Vec<NotificationItem>,
    pub unread: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivityFeedItem {
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub mode_slug: Option<String>,
    pub team_slug: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageItem {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub sender_username: String,
    pub sender_display_name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSendRequest {
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkNotificationsRequest {
    #[serde(default)]
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamInviteRequest {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub role: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamMemberRoleRequest {
    #[serde(default)]
    pub role: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TeamInvitationItem {
    pub id: Uuid,
    pub team_id: Uuid,
    pub team_slug: String,
    pub team_title: String,
    pub inviter_name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AuthorApplication {
    pub id: Uuid,
    pub message: String,
    pub links: String,
    pub status: String,
    pub note: String,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AuthorApplicationItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub message: String,
    pub links: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorApplicationRequest {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub links: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectAuthorApplicationRequest {
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseAppealItem {
    pub id: Uuid,
    pub release_id: Uuid,
    pub version: String,
    pub mode_title: String,
    pub mode_slug: String,
    pub author_name: String,
    pub author_username: String,
    pub message: String,
    pub rejection_note: Option<String>,
    pub created_at: DateTime<Utc>,
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
    pub scan: Option<serde_json::Value>,
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
    pub author_reply: Option<String>,
    pub author_reply_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewListResponse {
    pub average_rating: Option<f64>,
    pub count: i64,
    pub items: Vec<ReviewItem>,
    pub pending: bool,
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
pub struct ReviewReplyRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdminReviewItem {
    pub id: Uuid,
    pub mode_title: String,
    pub mode_slug: String,
    pub rating: i16,
    pub text: String,
    pub author_name: String,
    pub author_username: String,
    pub created_at: DateTime<Utc>,
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

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub username: Option<String>,
    pub display_name: Option<String>,
    #[serde(default = "default_true")]
    pub accept_terms: bool,
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
