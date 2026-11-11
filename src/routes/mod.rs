pub mod admin;
pub mod auth;
pub mod authors;
pub mod catalog;
pub mod chat;
pub mod community;
pub mod feed;
pub mod friends;
pub mod game_auth;
pub mod health;
pub mod news;
pub mod notifications;
pub mod players;
pub mod publisher;
pub mod teams;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/health", get(health::health))
        .route("/legal/terms", get(crate::legal::terms))
        .route("/legal/privacy", get(crate::legal::privacy))
        .route("/legal/rules", get(crate::legal::rules))
        .route("/legal/logo.png", get(crate::legal::logo))
        .route("/v1/metrics", get(health::metrics))
        .route("/v1/auth/register", post(auth::register))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/logout", post(auth::logout))
        .route(
            "/v1/auth/verify-email",
            get(auth::verify_email_page).post(auth::verify_email),
        )
        .route(
            "/v1/auth/verify-email/resend",
            post(auth::resend_verification),
        )
        .route("/v1/auth/password/forgot", post(auth::forgot_password))
        .route("/v1/auth/password/reset", post(auth::reset_password))
        .route("/reset-password", get(auth::reset_password_page))
        .route(
            "/v1/me/author-application",
            get(authors::status).post(authors::apply),
        )
        .route("/v1/me/teams", get(teams::mine))
        .route("/v1/me/profile", put(players::update_profile))
        .route("/v1/users/{username}", get(players::profile))
        .route("/v1/me/presence", post(friends::heartbeat))
        .route("/v1/me/friends", get(friends::list))
        .route(
            "/v1/me/friends/{username}",
            post(friends::add).delete(friends::remove),
        )
        .route("/v1/me/friend-requests", get(friends::requests))
        .route(
            "/v1/me/friend-requests/{id}/{action}",
            post(friends::respond),
        )
        .route(
            "/v1/me/notifications",
            get(notifications::list).post(notifications::mark_read),
        )
        .route("/v1/feed", get(feed::feed))
        .route(
            "/v1/chat/direct/{username}",
            get(chat::direct).post(chat::send_direct),
        )
        .route(
            "/v1/chat/teams/{id}",
            get(chat::team_messages).post(chat::send_team),
        )
        .route("/v1/me/team-invitations", get(teams::invitations))
        .route(
            "/v1/me/team-invitations/{id}/accept",
            post(teams::accept_invitation),
        )
        .route(
            "/v1/me/team-invitations/{id}/decline",
            post(teams::decline_invitation),
        )
        .route("/v1/teams", post(teams::create))
        .route(
            "/v1/teams/{slug}",
            get(teams::profile)
                .put(teams::update)
                .delete(teams::archive),
        )
        .route("/v1/teams/{id}/invitations", post(teams::invite))
        .route(
            "/v1/teams/{id}/join-requests",
            get(teams::join_requests).post(teams::request_join),
        )
        .route(
            "/v1/teams/{id}/join-requests/{request_id}/resolve",
            post(teams::resolve_join_request),
        )
        .route("/v1/teams/{id}/transfer", post(teams::transfer))
        .route(
            "/v1/teams/{id}/follow",
            post(teams::follow).delete(teams::unfollow),
        )
        .route("/v1/teams/{id}/news", post(teams::create_news))
        .route(
            "/v1/teams/{id}/members/{user_id}",
            put(teams::set_member_role).delete(teams::remove_member),
        )
        .route("/v1/me", get(auth::me))
        .route("/v1/me/identities", get(auth::identities))
        .route("/v1/game/session", post(game_auth::create_game_session))
        .route("/v1/game/logout", post(game_auth::revoke_game_sessions))
        .route(
            "/v1/me/skin",
            put(game_auth::upload_skin)
                .delete(game_auth::delete_skin)
                .layer(DefaultBodyLimit::max(2 * 1024 * 1024)),
        )
        .route("/authserver/authenticate", post(game_auth::authenticate))
        .route("/authserver/refresh", post(game_auth::refresh))
        .route("/authserver/validate", post(game_auth::validate))
        .route("/authserver/invalidate", post(game_auth::invalidate))
        .route("/authserver/signout", post(game_auth::signout))
        .route(
            "/sessionserver/session/minecraft/profile/{uuid}",
            get(game_auth::profile),
        )
        .route(
            "/sessionserver/session/minecraft/join",
            post(game_auth::join),
        )
        .route(
            "/sessionserver/session/minecraft/hasJoined",
            get(game_auth::has_joined),
        )
        .route("/v1/catalog", get(catalog::catalog))
        .route("/v1/collections", get(catalog::collections))
        .route(
            "/v1/news",
            get(news::store_news).post(news::create_store_news),
        )
        .route("/v1/news/{id}", delete(news::delete_news))
        .route("/v1/modes/{slug}", get(catalog::mode_detail))
        .route("/v1/modes/{slug}/news", get(news::mode_news))
        .route("/v1/modes/{slug}/releases", get(catalog::releases))
        .route("/v1/modes/{slug}/events", post(publisher::record_event))
        .route("/v1/modes/{slug}/reports", post(community::submit_report))
        .route(
            "/v1/modes/{slug}/reviews",
            get(community::reviews).post(community::submit_review),
        )
        .route(
            "/v1/modes/{slug}/reviews/{review_id}/reply",
            post(community::reply_review),
        )
        .route("/v1/me/wishlist", get(community::wishlist))
        .route(
            "/v1/me/wishlist/{slug}",
            post(community::add_wishlist).delete(community::remove_wishlist),
        )
        .route("/v1/me/follows", get(community::follows))
        .route("/v1/me/feed", get(community::feed))
        .route(
            "/v1/follows/{username}",
            post(community::follow_author).delete(community::unfollow_author),
        )
        .route("/v1/me/reviews/{slug}", delete(community::delete_review))
        .route("/v1/authors/{slug}", get(catalog::author))
        .route(
            "/v1/publisher/modes",
            get(publisher::my_modes).post(publisher::create_mode),
        )
        .route(
            "/v1/publisher/modes/{id}/releases",
            post(publisher::create_release),
        )
        .route(
            "/v1/publisher/modes/{id}/news",
            post(news::create_mode_news),
        )
        .route("/v1/publisher/modes/{id}/stats", get(publisher::stats))
        .route(
            "/v1/publisher/modes/{id}/cover",
            put(publisher::upload_cover)
                .delete(publisher::delete_cover)
                .layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route(
            "/v1/publisher/releases/{id}/assets/upload-url",
            post(publisher::upload_url),
        )
        .route(
            "/v1/publisher/releases/{release_id}/assets/{asset_id}/upload",
            put(publisher::upload_asset).layer(DefaultBodyLimit::max(512 * 1024 * 1024)),
        )
        .route(
            "/v1/publisher/releases/{release_id}/assets/{asset_id}/complete",
            post(publisher::complete_asset),
        )
        .route(
            "/v1/publisher/releases/{id}/submit",
            post(publisher::submit_release),
        )
        .route(
            "/v1/publisher/releases/{id}/appeal",
            post(publisher::appeal_release),
        )
        .route("/v1/admin/moderation/queue", get(admin::queue))
        .route("/v1/admin/moderation/history", get(admin::history))
        .route(
            "/v1/admin/collections",
            get(admin::collections).post(admin::create_collection),
        )
        .route(
            "/v1/admin/collections/{id}",
            delete(admin::delete_collection),
        )
        .route("/v1/admin/reviews", get(admin::reviews))
        .route(
            "/v1/admin/reviews/{id}/approve",
            post(admin::approve_review),
        )
        .route("/v1/admin/reviews/{id}/reject", post(admin::reject_review))
        .route("/v1/admin/reports", get(admin::reports))
        .route("/v1/admin/reports/{id}/close", post(admin::close_report))
        .route("/v1/admin/appeals", get(admin::appeals))
        .route(
            "/v1/admin/appeals/{id}/resolve",
            post(admin::resolve_appeal),
        )
        .route("/v1/admin/author-applications", get(authors::admin_list))
        .route(
            "/v1/admin/author-applications/{id}/approve",
            post(authors::admin_approve),
        )
        .route(
            "/v1/admin/author-applications/{id}/reject",
            post(authors::admin_reject),
        )
        .route("/v1/admin/teams/{id}/verify", post(teams::admin_verify))
        .route("/v1/admin/moderation/{id}/approve", post(admin::approve))
        .route("/v1/admin/moderation/{id}/reject", post(admin::reject))
        .route("/v1/admin/modes", get(admin::modes))
        .route("/v1/admin/modes/{id}/hide", post(admin::hide_mode))
        .route("/v1/admin/modes/{id}/unhide", post(admin::unhide_mode))
}

pub fn validate_slug(value: &str) -> bool {
    let length = value.len();
    (3..=64).contains(&length)
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}
