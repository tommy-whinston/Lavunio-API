use axum::extract::Query;
use axum::response::Html;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LegalQuery {
    pub lang: Option<String>,
}

fn english(query: &LegalQuery) -> bool {
    query.lang.as_deref().map(|value| value.eq_ignore_ascii_case("en")).unwrap_or(false)
}

fn page(title: &str, body: &str, english: bool) -> String {
    let lang = if english { "en" } else { "ru" };
    let nav = if english {
        r#"<nav><a href="/legal/terms?lang=en">Terms</a><a href="/legal/privacy?lang=en">Privacy</a><a href="/legal/rules?lang=en">Rules</a><a href="?lang=ru">Русский</a></nav>"#
    } else {
        r#"<nav><a href="/legal/terms">Условия</a><a href="/legal/privacy">Конфиденциальность</a><a href="/legal/rules">Правила</a><a href="?lang=en">English</a></nav>"#
    };
    format!(
        r#"<!doctype html><html lang="{lang}"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title} — Lavunio</title><style>
body{{margin:0;background:#0f1115;color:#d8dee4;font:15px/1.65 "Segoe UI",Arial,sans-serif}}
main{{max-width:820px;margin:0 auto;padding:48px 24px 80px}}
nav{{display:flex;flex-wrap:wrap;gap:16px;margin-bottom:32px}}
a{{color:#58a6ff;text-decoration:none}}a:hover{{text-decoration:underline}}
h1{{color:#f0f6fc;font-size:30px;margin:0 0 8px}}h2{{color:#f0f6fc;font-size:19px;margin:32px 0 10px}}
.brand{{color:#7ee787;font-weight:700;letter-spacing:3px;margin-bottom:24px}}
.updated{{color:#8b949e;font-size:12px;margin-bottom:28px}}
li{{margin:6px 0}}p{{margin:10px 0}}
</style></head><body><main><div class="brand">LAVUNIO</div>{nav}<h1>{title}</h1><div class="updated">2026-11-09</div>{body}</main></body></html>"#
    )
}

pub async fn terms(Query(query): Query<LegalQuery>) -> Html<String> {
    let en = english(&query);
    if en {
        Html(page("Terms of Use", TERMS_EN, true))
    } else {
        Html(page("Пользовательское соглашение", TERMS_RU, false))
    }
}

pub async fn privacy(Query(query): Query<LegalQuery>) -> Html<String> {
    let en = english(&query);
    if en {
        Html(page("Privacy Policy", PRIVACY_EN, true))
    } else {
        Html(page("Политика конфиденциальности", PRIVACY_RU, false))
    }
}

pub async fn rules(Query(query): Query<LegalQuery>) -> Html<String> {
    let en = english(&query);
    if en {
        Html(page("Community Rules", RULES_EN, true))
    } else {
        Html(page("Правила сообщества", RULES_RU, false))
    }
}

const TERMS_RU: &str = r#"
<p>Настоящие условия регулируют использование лаунчера Lavunio, каталога режимов, сайта и связанных сервисов.</p>
<h2>1. Аккаунт</h2>
<ul>
<li>Для публикации и социальных функций нужен аккаунт Lavunio.</li>
<li>Указывайте достоверные данные и не передавайте пароль третьим лицам.</li>
<li>Вы отвечаете за действия, совершённые под вашим аккаунтом.</li>
</ul>
<h2>2. Контент авторов</h2>
<ul>
<li>Автор гарантирует, что у него есть права на сборку, обложки, тексты и другие материалы.</li>
<li>Загружая контент, автор разрешает Lavunio хранить, отображать и распространять его в рамках платформы.</li>
<li>Lavunio вправе отклонить, скрыть или удалить контент, нарушающий правила или законодательство.</li>
</ul>
<h2>3. Запрещено</h2>
<ul>
<li>вредоносный код, вирусы, эксплойты и обход защиты;</li>
<li>незаконный контент, пиратские сборки и материалы без прав;</li>
<li>оскорбления, угрозы, травля и разжигание вражды;</li>
<li>спам, мошенничество и выдача себя за другого автора или команду.</li>
</ul>
<h2>4. Модерация</h2>
<p>Релизы и комментарии могут проходить проверку. Решение модератора можно оспорить через апелляцию в лаунчере.</p>
<h2>5. Ответственность</h2>
<p>Сервис предоставляется «как есть». Мы не гарантируем бесперебойную работу и не отвечаем за контент, загруженный пользователями. Мы стремимся исправлять ошибки и поддерживать безопасность.</p>
<h2>6. Блокировка</h2>
<p>Аккаунт может быть ограничен или заблокирован за нарушение условий, правил или законодательства.</p>
<h2>7. Изменения</h2>
<p>Мы можем обновлять условия. Новая версия публикуется на этой странице.</p>
<h2>8. Контакты</h2>
<p>Вопросы: <a href="mailto:support@lavunio.pro">support@lavunio.pro</a>.</p>
"#;

const TERMS_EN: &str = r#"
<p>These terms govern the use of the Lavunio launcher, mode catalog, website and related services.</p>
<h2>1. Account</h2>
<ul>
<li>A Lavunio account is required for publishing and social features.</li>
<li>Provide accurate information and do not share your password.</li>
<li>You are responsible for activity performed under your account.</li>
</ul>
<h2>2. Author content</h2>
<ul>
<li>Authors confirm they hold the rights to builds, covers, text and other materials.</li>
<li>By uploading content, authors allow Lavunio to store, display and distribute it within the platform.</li>
<li>Lavunio may reject, hide or remove content that violates these terms or the law.</li>
</ul>
<h2>3. Prohibited</h2>
<ul>
<li>malware, exploits and protection bypasses;</li>
<li>illegal content, pirated builds and unlicensed materials;</li>
<li>harassment, threats, hate speech and abuse;</li>
<li>spam, fraud and impersonation of another author or team.</li>
</ul>
<h2>4. Moderation</h2>
<p>Releases and comments may be reviewed. Moderation decisions can be appealed in the launcher.</p>
<h2>5. Liability</h2>
<p>The service is provided “as is”. We do not guarantee uninterrupted operation and are not responsible for user-uploaded content. We work to fix issues and keep the platform secure.</p>
<h2>6. Suspension</h2>
<p>Accounts may be restricted or blocked for violating these terms, community rules or the law.</p>
<h2>7. Changes</h2>
<p>We may update these terms. The new version is published on this page.</p>
<h2>8. Contact</h2>
<p>Questions: <a href="mailto:support@lavunio.pro">support@lavunio.pro</a>.</p>
"#;

const PRIVACY_RU: &str = r#"
<p>Политика описывает, какие данные Lavunio собирает и как их использует.</p>
<h2>1. Какие данные мы собираем</h2>
<ul>
<li>email, username, отображаемое имя и хеш пароля;</li>
<li>Minecraft UUID, скины и плащи;</li>
<li>сессии, IP-адрес и user-agent при входе;</li>
<li>комментарии, отзывы, сообщения чата, заявки и действия в командах;</li>
<li>технические логи API и ошибок.</li>
</ul>
<h2>2. Зачем</h2>
<ul>
<li>вход, регистрация и восстановление доступа;</li>
<li>работа каталога, библиотеки, публикатора и социальных функций;</li>
<li>модерация, безопасность и поддержка;</li>
<li>улучшение стабильности сервиса.</li>
</ul>
<h2>3. Где хранятся данные</h2>
<p>Основная база — PostgreSQL на VPS Aeza. Файлы сборок, обложек и скинов — S3 Timeweb. Письма отправляются через Resend. Мы не продаём данные третьим лицам.</p>
<h2>4. Срок хранения</h2>
<p>Данные аккаунта хранятся до удаления аккаунта. Резервные копии базы — до 7 дней. Технические логи — ограниченное время, необходимое для диагностики и безопасности.</p>
<h2>5. Ваши права</h2>
<p>Вы можете запросить доступ, исправление или удаление данных, написав на <a href="mailto:support@lavunio.pro">support@lavunio.pro</a>.</p>
<h2>6. Cookies и аналитика</h2>
<p>Лаунчер не использует рекламные cookies и стороннюю аналитику. Юридические страницы не содержат трекеров.</p>
<h2>7. Безопасность</h2>
<p>Пароли хранятся в виде хешей Argon2id, передача идёт по HTTPS, доступ к данным ограничен.</p>
<h2>8. Дети</h2>
<p>Сервис не предназначен для детей младше 13 лет.</p>
<h2>9. Изменения</h2>
<p>Новая версия политики публикуется на этой странице.</p>
"#;

const PRIVACY_EN: &str = r#"
<p>This policy explains what data Lavunio collects and how it is used.</p>
<h2>1. Data we collect</h2>
<ul>
<li>email, username, display name and password hash;</li>
<li>Minecraft UUID, skins and capes;</li>
<li>sessions, IP address and user agent on sign-in;</li>
<li>comments, reviews, chat messages, applications and team activity;</li>
<li>API and error logs.</li>
</ul>
<h2>2. Why</h2>
<ul>
<li>sign-in, registration and account recovery;</li>
<li>catalog, library, publisher and social features;</li>
<li>moderation, security and support;</li>
<li>service stability.</li>
</ul>
<h2>3. Where data is stored</h2>
<p>The main database is PostgreSQL on an Aeza VPS. Builds, covers and skins are stored in Timeweb S3. Email is delivered via Resend. We do not sell data to third parties.</p>
<h2>4. Retention</h2>
<p>Account data is kept until account deletion. Database backups are kept for up to 7 days. Technical logs are retained for the limited time needed for diagnostics and security.</p>
<h2>5. Your rights</h2>
<p>You may request access, correction or deletion of your data by writing to <a href="mailto:support@lavunio.pro">support@lavunio.pro</a>.</p>
<h2>6. Cookies and analytics</h2>
<p>The launcher uses no advertising cookies or third-party analytics. Legal pages contain no trackers.</p>
<h2>7. Security</h2>
<p>Passwords are stored as Argon2id hashes, traffic uses HTTPS, and data access is restricted.</p>
<h2>8. Children</h2>
<p>The service is not intended for children under 13.</p>
<h2>9. Changes</h2>
<p>A new version of this policy is published on this page.</p>
"#;

const RULES_RU: &str = r#"
<p>Правила действуют для всех пользователей, авторов и команд Lavunio.</p>
<h2>1. Запрещённый контент</h2>
<ul>
<li>вредоносные файлы, вирусы, стилеры и эксплойты;</li>
<li>пиратские сборки, моды и материалы без разрешения правообладателя;</li>
<li>незаконный контент и пропаганда;</li>
<li>оскорбления, угрозы, травля, разжигание вражды;</li>
<li>спам, накрутка, мошенничество, выдача себя за других.</li>
</ul>
<h2>2. Правила авторов</h2>
<ul>
<li>указывайте честные описание, версию и состав сборки;</li>
<li>не загружайте чужие сборки без разрешения;</li>
<li>не добавляйте скрытые исполняемые файлы и подозрительный код;</li>
<li>поддерживайте релизы и отвечайте на обращения модераторов.</li>
</ul>
<h2>3. Правила общения</h2>
<ul>
<li>уважайте других игроков и авторов;</li>
<li>не публикуйте личные данные без согласия;</li>
<li>жалуйтесь на нарушения через кнопку «Пожаловаться», а не устраивайте конфликты.</li>
</ul>
<h2>4. Модерация и санкции</h2>
<p>Модератор может отклонить релиз, скрыть режим, удалить комментарий или ограничить аккаунт. Санкции зависят от нарушения и его повторяемости.</p>
<h2>5. Апелляции</h2>
<p>Отклонённый релиз можно оспорить в разделе «Публикация». Решение по апелляции принимает модератор.</p>
<h2>6. Контакты</h2>
<p>Жалобы и вопросы: <a href="mailto:support@lavunio.pro">support@lavunio.pro</a>.</p>
"#;

const RULES_EN: &str = r#"
<p>These rules apply to all Lavunio users, authors and teams.</p>
<h2>1. Prohibited content</h2>
<ul>
<li>malware, viruses, stealers and exploits;</li>
<li>pirated builds, mods and unlicensed materials;</li>
<li>illegal content and propaganda;</li>
<li>harassment, threats, hate speech and abuse;</li>
<li>spam, manipulation, fraud and impersonation.</li>
</ul>
<h2>2. Author rules</h2>
<ul>
<li>provide honest descriptions, versions and build contents;</li>
<li>do not upload other people’s builds without permission;</li>
<li>do not include hidden executables or suspicious code;</li>
<li>maintain releases and respond to moderator requests.</li>
</ul>
<h2>3. Communication</h2>
<ul>
<li>respect other players and authors;</li>
<li>do not publish personal data without consent;</li>
<li>report violations with the “Report” button instead of starting conflicts.</li>
</ul>
<h2>4. Moderation and sanctions</h2>
<p>Moderators may reject a release, hide a mode, delete a comment or restrict an account. Sanctions depend on the violation and whether it repeats.</p>
<h2>5. Appeals</h2>
<p>A rejected release can be appealed in the Publisher section. A moderator reviews the appeal.</p>
<h2>6. Contact</h2>
<p>Reports and questions: <a href="mailto:support@lavunio.pro">support@lavunio.pro</a>.</p>
"#;
