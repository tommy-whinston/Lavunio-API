# Lavunio API

[![CI](https://github.com/tommy-whinston/Lavunio-API/actions/workflows/ci.yml/badge.svg)](https://github.com/tommy-whinston/Lavunio-API/actions/workflows/ci.yml)

Локальный backend Lavunio: каталог режимов, релизы, авторы, черновики, загрузка файлов и минимальная модерация.

Текущая версия — **MVP для локальной разработки**. Целевая production-схема: PostgreSQL + R2/S3 + CDN.

## Что уже есть

- `GET /v1/health` — состояние API и базы.
- `GET /v1/catalog` — каталог опубликованных режимов с поиском, тегом и пагинацией.
- `GET /v1/modes/{slug}` — карточка режима.
- `GET /v1/modes/{slug}/releases` — опубликованные релизы с файлами.
- `GET /v1/authors/{slug}` — профиль автора и его режимы.
- `POST /v1/publisher/modes` — создать черновик режима.
- `POST /v1/publisher/modes/{id}/releases` — создать черновик релиза.
- `POST /v1/publisher/releases/{id}/assets/upload-url` — получить URL для загрузки файла.
- `PUT /v1/publisher/releases/{release_id}/assets/{asset_id}/upload` — загрузить файл.
- `POST /v1/publisher/releases/{id}/submit` — отправить релиз на модерацию.
- `GET /v1/admin/moderation/queue` — очередь модерации.
- `POST /v1/admin/moderation/{id}/approve` — одобрить релиз.
- `POST /v1/admin/moderation/{id}/reject` — отклонить релиз.
- Seed-скрипт для переноса `night-demon` и `eclipse`.
- CLI для создания API-токенов.

## Требования

- Rust 1.80+.
- PostgreSQL 16.
- Docker Desktop или локальный PostgreSQL.
- MinIO — опционально, для будущего S3-адаптера.

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

Пример создания черновика:

```powershell
curl -X POST http://127.0.0.1:8080/v1/publisher/modes `
  -H "Authorization: Bearer <token>" `
  -H "Content-Type: application/json" `
  -d '{\"slug\":\"my-mode\",\"title\":\"My Mode\",\"shortDescription\":\"Test\",\"tags\":[\"pvp\"]}'
```

## Файлы

Сейчас файлы сохраняются локально в `data/uploads/` и отдаются по `/uploads/{assetId}`. Это временная замена R2/S3 для локальной разработки. В production `POST /assets/upload-url` будет возвращать pre-signed URL Cloudflare R2, а API не будет проксировать большие файлы через себя.

`docker-compose.yml` уже содержит MinIO — он понадобится на следующем шаге.

## Структура

```text
src/
  main.rs          запуск Axum
  lib.rs           общие модули и AppState
  config.rs        переменные окружения
  db.rs            пул PostgreSQL и миграции
  auth.rs          API-токены, роли, bootstrap
  error.rs         JSON-ошибки
  models.rs        модели API
  storage.rs       локальное хранилище файлов
  routes/
    health.rs
    catalog.rs
    publisher.rs
    admin.rs
migrations/
  0001_init.sql
seed/
  modes.json
  *.manifest.json
```

## Следующие шаги

- Перевести хранилище на Cloudflare R2 + pre-signed URL.
- Добавить Ely.by-авторизацию игроков.
- Добавить отзывы, жалобы, подписки и библиотеку.
- Подключить лаунчер к API вместо GitHub-каталога.
- Добавить интеграционные тесты с PostgreSQL.

## Планы

- Аккаунты Lavunio и провайдеры скинов: [docs/ACCOUNTS.md](https://github.com/tommy-whinston/Lavunio/blob/main/docs/ACCOUNTS.md)
- Встроенный публикатор в лаунчере: [docs/PUBLISHER.md](https://github.com/tommy-whinston/Lavunio/blob/main/docs/PUBLISHER.md)
