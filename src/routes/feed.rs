use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::error::ApiError;
use crate::models::ActivityFeedItem;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    pub limit: Option<i64>,
}

pub async fn feed(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<Vec<ActivityFeedItem>>, ApiError> {
    let limit = query.limit.unwrap_or(30).clamp(1, 100);
    let items = sqlx::query_as::<_, ActivityFeedItem>(
        r#"
        SELECT kind, title, subtitle, username, display_name, avatar_url,
               mode_slug, team_slug, created_at
        FROM (
            SELECT 'release' AS kind,
                   m.title || ' · ' || r.version AS title,
                   COALESCE(r.changelog, '') AS subtitle,
                   u.username, u.display_name, u.avatar_url,
                   m.slug AS mode_slug, NULL::text AS team_slug,
                   COALESCE(r.published_at, r.created_at) AS created_at
            FROM releases r
            JOIN modes m ON m.id = r.mode_id
            JOIN users u ON u.id = m.author_id
            WHERE r.status = 'published' AND m.status = 'published'

            UNION ALL

            SELECT 'review',
                   m.title,
                   left(rv.text, 160),
                   u.username, u.display_name, u.avatar_url,
                   m.slug, NULL::text,
                   rv.created_at
            FROM reviews rv
            JOIN modes m ON m.id = rv.mode_id
            JOIN users u ON u.id = rv.user_id
            WHERE rv.status = 'published' AND m.status = 'published'

            UNION ALL

            SELECT 'team_news',
                   tn.title,
                   left(tn.body, 160),
                   u.username, u.display_name, u.avatar_url,
                   NULL::text, t.slug,
                   tn.created_at
            FROM team_news tn
            JOIN teams t ON t.id = tn.team_id
            JOIN users u ON u.id = tn.author_id
            WHERE tn.status = 'published' AND t.status = 'active'
        ) feed
        ORDER BY created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(items))
}
