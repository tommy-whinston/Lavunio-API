# Lavunio API

[![CI](https://github.com/tommy-whinston/Lavunio-API/actions/workflows/ci.yml/badge.svg)](https://github.com/tommy-whinston/Lavunio-API/actions/workflows/ci.yml)

Локальный backend Lavunio: каталог режимов, релизы, авторы, черновики, загрузка файлов и минимальная модерация.

Текущая версия — **MVP для локальной разработки**. Целевая production-схема: PostgreSQL + R2/S3 + CDN.

## Что уже есть

- `GET /v1/health` — состояние API и базы.
- `GET /v1/metrics` — метрики Prometheus: uptime, запросы, ошибки, БД, очередь модерации.
- `POST /v1/auth/register` — регистрация по email и паролю (Argon2id).
- `POST /v1/auth/login` — вход, access/refresh-токены.
- `POST /v1/auth/refresh` — обновление пары токенов.
- `POST /v1/auth/logout` — отзыв текущей сессии.
- `POST /v1/auth/verify-email` — подтверждение почты по токену.
- `POST /v1/auth/verify-email/resend` — повторная отправка подтверждения.
- `POST /v1/auth/password/forgot` — запрос сброса пароля.
- `POST /v1/auth/password/reset` — сброс пароля по токену.
- `GET /v1/me` — профиль текущего аккаунта.
- `GET /v1/me/identities` — привязанные внешние аккаунты.
- `POST /v1/game/session` — игровая сессия Minecraft для authlib-injector.
- `PUT /v1/me/skin` — загрузить скин Lavunio; `DELETE /v1/me/skin` — удалить.
- `PUT /v1/me/cape` — загрузить плащ; `DELETE /v1/me/cape` — удалить.
- `/authserver/*` и `/sessionserver/*` — Yggdrasil-совместимые endpoints для Minecraft.
- `GET /v1/catalog` — каталог опубликованных режимов с поиском, тегом и пагинацией.
- `GET /v1/collections` — подборки: рекомендуем, популярное, новинки, обновления.
  - Параметры: `q`, `tag`, `mcVersion`, `loader`, `author`, `sort` (`updated`, `newest`, `title`, `installs`), `page`, `limit`.
  - Элемент каталога содержит `installs` и `screenshots` последнего опубликованного релиза.
- `GET /v1/modes/{slug}` — карточка режима.
- `GET /v1/modes/{slug}/releases` — опубликованные релизы с файлами.
- `GET /v1/modes/{slug}/reviews` — опубликованные отзывы и средний рейтинг.
- `POST /v1/modes/{slug}/reviews` — оставить или обновить свой отзыв.
- `DELETE /v1/me/reviews/{slug}` — удалить свой отзыв.
- `POST /v1/modes/{slug}/reports` — пожаловаться на режим.
- `GET /v1/me/follows` — авторы, на которых подписан игрок.
- `POST /v1/follows/{username}` — подписаться на автора; `DELETE` — отписаться.
- `GET /v1/me/feed` — последние релизы авторов из подписок.
- `GET /v1/admin/reports?status=open` — очередь жалоб.
- `POST /v1/admin/reports/{id}/close` — закрыть жалобу.
- `POST /v1/modes/{slug}/events` — события скачивания, установки и обновления для статистики.
- `GET /v1/me/wishlist` — список желаемого.
- `POST /v1/me/wishlist/{slug}` — добавить в список желаемого; `DELETE` — убрать.
- `GET /v1/authors/{slug}` — профиль автора и его режимы.
- `GET /v1/publisher/modes` — режимы текущего автора с релизами, файлами и статистикой.
- `POST /v1/publisher/modes` — создать черновик режима.
- `PUT /v1/publisher/modes/{id}/cover` — загрузить обложку файлом; `DELETE` — убрать.
- `GET /v1/publisher/modes/{id}/stats` — статистика режима.
- `POST /v1/publisher/modes/{id}/releases` — создать черновик релиза.
- `POST /v1/publisher/releases/{id}/assets/upload-url` — получить URL для загрузки файла (local или S3/R2 pre-signed); принимает манифест сборки.
- `PUT /v1/publisher/releases/{release_id}/assets/{asset_id}/upload` — загрузить файл через API (local storage).
- `POST /v1/publisher/releases/{release_id}/assets/{asset_id}/complete` — подтвердить загрузку в S3/R2.
- `POST /v1/publisher/releases/{id}/submit` — отправить релиз на модерацию.
- `GET /v1/admin/moderation/queue` — очередь модерации.
- `POST /v1/admin/moderation/{id}/approve` — одобрить релиз.
- `POST /v1/admin/moderation/{id}/reject` — отклонить релиз с причиной.
- `GET /v1/admin/moderation/history` — журнал действий модераторов.
- `GET /v1/admin/modes` — все режимы для админки.
- `POST /v1/admin/modes/{id}/hide` — скрыть режим с причиной.
- `POST /v1/admin/modes/{id}/unhide` — вернуть режим в каталог.
- Seed-скрипт для переноса `night-demon` и `eclipse`.
- CLI для создания API-токенов.

## Требования

- Rust 1.80+.
- PostgreSQL 16.
- Docker Desktop или локальный PostgreSQL.
- S3-совместимое хранилище (Cloudflare R2, MinIO, AWS S3) — для production.

## Быстрый старт

1. Скопируй `.env.example` в `.env` и при необходимости поменяй значения.
2. Подними PostgreSQL:

```powershell
docker compose up -d db
```

3. Запусти API:

```powershell
cargo run
```

При старте API попробует применить миграции. Если база недоступна, API всё равно запустится, но каталог будет отвечать ошибкой, а `/v1/health` покажет `"database": "down"`.

4. Загрузи текущие режимы:

```powershell
cargo run --bin seed
```

5. Создай токен автора:

```powershell
cargo run --bin token -- tm "Local publisher" publisher
```

Для модерации удобно использовать bootstrap-токен из `.env`:
`LAVUNIO_BOOTSTRAP_TOKEN=...` создаст пользователя `tm` с ролью `admin`.

## Проверка

```powershell
curl http://127.0.0.1:8080/v1/health
curl "http://127.0.0.1:8080/v1/catalog?page=1&limit=20"
curl http://127.0.0.1:8080/v1/modes/night-demon
```

Авторизация:

```text
Authorization: Bearer <token>
```

Подходят и API-токены авторов, и access-токены аккаунтов Lavunio.

## Аккаунты Lavunio

Аккаунты используют email и пароль. Пароли хранятся как Argon2id-хеши, access- и refresh-токены — только в виде SHA-256-хешей в таблице `sessions`. Игровые токены Minecraft хранятся в `game_sessions`.

Корень API (`GET /`) содержит metadata для `authlib-injector`: имя сервера и `skinDomains`. Yggdrasil-совместимые endpoints находятся в `/authserver` и `/sessionserver`.

Скины и плащи загружаются через `PUT /v1/me/skin` и `PUT /v1/me/cape`, отдаются из storage по `/uploads/skins/...`.

Для локальной разработки включи `LAVUNIO_DEV_MODE=1`: API вернёт `verificationToken` и `resetToken` в ответах, а также напишет их в лог. В production эти поля не возвращаются — токены должны уходить письмом.

Переменные:

- `LAVUNIO_ACCESS_TTL_SECS` — срок access-токена, по умолчанию 900 секунд.
- `LAVUNIO_REFRESH_TTL_SECS` — срок refresh-токена, по умолчанию 30 дней.
- `LAVUNIO_EMAIL_TTL_SECS` — срок подтверждения почты, по умолчанию 24 часа.
- `LAVUNIO_RESET_TTL_SECS` — срок сброса пароля, по умолчанию 1 час.
- `LAVUNIO_GAME_TTL_SECS` — срок игровой сессии Minecraft, по умолчанию 30 дней.
- `SKIN_DOMAINS` — домены, с которых Minecraft может загружать скины (через запятую).
- `LAVUNIO_RATE_LIMIT_ENABLED` — включить rate limit (по умолчанию `true`).
- `LAVUNIO_RATE_LIMIT_WINDOW_SECS` — окно rate limit, по умолчанию 900 секунд.
- `LAVUNIO_RATE_LIMIT_LOGIN` — попыток входа на email/IP за окно, по умолчанию 10.
- `LAVUNIO_RATE_LIMIT_REGISTER` — регистраций на IP за окно, по умолчанию 10.
- `LAVUNIO_RATE_LIMIT_RESET` — запросов сброса пароля на IP за окно, по умолчанию 5.

Rate limit хранится в памяти процесса. Для нескольких инстансов API понадобится общий store вроде Redis.

## Тесты

Интеграционные тесты используют PostgreSQL и создают временные БД:

```powershell
$env:DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres"
cargo test --all-targets
```

Пример создания черновика:

```powershell
curl -X POST http://127.0.0.1:8080/v1/publisher/modes `
  -H "Authorization: Bearer <token>" `
  -H "Content-Type: application/json" `
  -d '{\"slug\":\"my-mode\",\"title\":\"My Mode\",\"shortDescription\":\"Test\",\"tags\":[\"pvp\"]}'
```

## Мониторинг и логи

`GET /v1/health` возвращает статус БД, задержку, uptime, storage и размер пула.

`GET /v1/metrics` отдаёт Prometheus-метрики:

```text
lavunio_uptime_seconds
lavunio_requests_total
lavunio_errors_total
lavunio_db_up
lavunio_db_pool_size
lavunio_db_pool_idle
lavunio_pending_releases
```

Каждый HTTP-запрос пишется в лог с полями `method`, `path`, `status`, `latency_ms`.

По умолчанию логи текстовые. Для JSON:

```env
LOG_FORMAT=json
```

## Бэкапы PostgreSQL

Скрипты используют `pg_dump` и `pg_restore` из PostgreSQL. Добавь `bin` PostgreSQL в `PATH` или установи PostgreSQL в стандартную папку.

Бэкап:

```powershell
.\scripts\backup-db.ps1 -KeepDays 7
```

Файлы сохраняются в `backups/lavunio-YYYYMMDD-HHMMSS.dump`. Старые бэкапы удаляются автоматически.

Восстановление (останови API):

```powershell
.\scripts\restore-db.ps1 -Dump .\backups\lavunio-20261108-120000.dump
```

Скрипт использует `--clean --if-exists`, то есть заменяет объекты в целевой базе.

## Файлы и S3
По умолчанию файлы сохраняются локально в `data/uploads/` и отдаются по `/uploads/{assetId}`.

Для S3/R2:

```env
STORAGE_KIND=s3
S3_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com
S3_REGION=auto
S3_BUCKET=lavunio
S3_ACCESS_KEY_ID=...
S3_SECRET_ACCESS_KEY=...
S3_PUBLIC_BASE_URL=https://cdn.example.com
S3_FORCE_PATH_STYLE=true
S3_PRESIGN_EXPIRES_SECS=3600
```

В S3-режиме `upload-url` возвращает pre-signed `PUT` URL и `completeUrl`. После загрузки клиент вызывает `complete`, API проверяет объект через `HEAD` и сохраняет публичный URL. API не проксирует большие файлы через себя.

`docker-compose.yml` содержит MinIO для локальной проверки S3-режима.

## Миграция файлов в S3/R2

Скрипт `migrate_assets` скачивает текущие файлы из GitHub-каталога, загружает их в S3/R2 и обновляет `release_assets`:

```powershell
# Заполни S3_* в .env и включи STORAGE_KIND=s3
cargo run --bin migrate_assets
```

Для split-архивов (`build.zip.001`, `.002`, ...) скрипт загружает все части и записывает публичные URL в `parts`.

## Структура

```text
src/
  main.rs          запуск Axum
  lib.rs           общие модули и AppState
  config.rs        переменные окружения
  db.rs            пул PostgreSQL и миграции
  auth.rs          API-токены, аккаунты, сессии, пароли
  error.rs         JSON-ошибки
  models.rs        модели API
  storage.rs       локальное хранилище файлов
  rate_limit.rs    rate limit login/register/reset
  metrics.rs       счётчики запросов и uptime
  routes/
    health.rs
    auth.rs
    game_auth.rs
    catalog.rs
    community.rs
    publisher.rs
    admin.rs
migrations/
  0001_init.sql
  0002_storage_keys.sql
  0003_mode_meta.sql
  0004_accounts.sql
  0005_account_links.sql
  0006_skins.sql
  0007_moderation.sql
  0008_mode_events.sql
  0009_wishlist.sql
seed/
  modes.json
  *.manifest.json
scripts/
  backup-db.ps1
  restore-db.ps1
tests/
  api.rs
  auth.rs
  game_auth.rs
  storage.rs
```

## Бинарники

- `cargo run` — API.
- `cargo run --bin seed` — импорт текущих режимов.
- `cargo run --bin token -- <username> <name>` — создать API-токен.
- `cargo run --bin role -- <email|username> <role>` — выдать роль игрока, автора, модератора или админа.
- `cargo run --bin migrate_assets` — перенести файлы из GitHub в S3/R2.

## Следующие шаги

- Подключить отправку писем для подтверждения почты и сброса пароля.
- Добавить внешний алертинг при падении API.
- Проверить вход в Minecraft через authlib-injector на реальном сервере.
- Добавить отзывы, жалобы, подписки и библиотеку.
- Подключить аккаунты к магазину и социальным функциям.

## Планы

- Аккаунты Lavunio и провайдеры скинов: [docs/ACCOUNTS.md](https://github.com/tommy-whinston/Lavunio/blob/main/docs/ACCOUNTS.md)
- Авторы и команды: [docs/TEAMS.md](https://github.com/tommy-whinston/Lavunio/blob/main/docs/TEAMS.md)
- Встроенный публикатор в лаунчере: [docs/PUBLISHER.md](https://github.com/tommy-whinston/Lavunio/blob/main/docs/PUBLISHER.md)
