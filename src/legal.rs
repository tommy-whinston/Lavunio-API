use axum::extract::Query;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LegalQuery {
    pub lang: Option<String>,
}

fn english(query: &LegalQuery) -> bool {
    query
        .lang
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("en"))
        .unwrap_or(false)
}

pub async fn logo() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=604800"),
        ],
        include_bytes!("legal_logo.png").as_slice(),
    )
}

pub async fn terms(Query(query): Query<LegalQuery>) -> Html<String> {
    let en = english(&query);
    if en {
        Html(page(
            "en",
            "Terms of Use",
            "These terms govern the use of the Lavunio launcher, mode catalog, website and related services. By using Lavunio you confirm that you have read and accepted them.",
            "terms",
            "/legal/terms",
            TERMS_EN,
            TERMS_TOC_EN,
        ))
    } else {
        Html(page(
            "ru",
            "Пользовательское соглашение",
            "Настоящее Соглашение определяет правила использования лаунчера Lavunio, каталога режимов, сайта lavunio.pro и связанных сервисов. Используя Lavunio, вы подтверждаете, что ознакомились с Соглашением и принимаете его условия.",
            "terms",
            "/legal/terms",
            TERMS_RU,
            TERMS_TOC_RU,
        ))
    }
}

pub async fn privacy(Query(query): Query<LegalQuery>) -> Html<String> {
    let en = english(&query);
    if en {
        Html(page(
            "en",
            "Privacy Policy",
            "This policy explains what data Lavunio collects, why we need it, where it is stored and what rights you have.",
            "privacy",
            "/legal/privacy",
            PRIVACY_EN,
            PRIVACY_TOC_EN,
        ))
    } else {
        Html(page(
            "ru",
            "Политика конфиденциальности",
            "Политика объясняет, какие данные Lavunio собирает, зачем они нужны, где хранятся и какие права есть у пользователей.",
            "privacy",
            "/legal/privacy",
            PRIVACY_RU,
            PRIVACY_TOC_RU,
        ))
    }
}

pub async fn rules(Query(query): Query<LegalQuery>) -> Html<String> {
    let en = english(&query);
    if en {
        Html(page(
            "en",
            "Community Rules",
            "These rules apply to all Lavunio users, authors and teams. They supplement the Terms of Use and help keep the platform safe.",
            "rules",
            "/legal/rules",
            RULES_EN,
            RULES_TOC_EN,
        ))
    } else {
        Html(page(
            "ru",
            "Правила сообщества",
            "Правила действуют для всех пользователей, авторов и команд Lavunio. Они дополняют Пользовательское соглашение и помогают поддерживать безопасность платформы.",
            "rules",
            "/legal/rules",
            RULES_RU,
            RULES_TOC_RU,
        ))
    }
}

fn page(
    lang: &str,
    title: &str,
    lead: &str,
    active: &str,
    path: &str,
    body: &str,
    toc: &str,
) -> String {
    let en = lang == "en";
    let (terms, privacy, rules, toc_title, support, effective, version, home, rights) = if en {
        (
            "Terms",
            "Privacy",
            "Rules",
            "Contents",
            "Support",
            "Effective November 9, 2026",
            "Version 1.0",
            "Home",
            "All rights reserved.",
        )
    } else {
        (
            "Условия",
            "Конфиденциальность",
            "Правила",
            "Содержание",
            "Поддержка",
            "Действует с 9 ноября 2026 г.",
            "Версия 1.0",
            "На главную",
            "Все права защищены.",
        )
    };
    let (skip, nav_label, lang_label) = if en {
        ("Skip to content", "Legal documents", "Language")
    } else {
        ("К содержанию", "Юридические документы", "Язык")
    };
    let active_terms = if active == "terms" { " active" } else { "" };
    let active_privacy = if active == "privacy" { " active" } else { "" };
    let active_rules = if active == "rules" { " active" } else { "" };
    let ru_active = if en { "" } else { " active" };
    let en_active = if en { " active" } else { "" };
    let lang_query = if en { "en" } else { "ru" };
    let footer_note = if en {
        "Lavunio is a platform for Minecraft game modes. Legal documents are published in Russian and English."
    } else {
        "Lavunio — платформа игровых режимов Minecraft. Юридические документы публикуются на русском и английском языках."
    };
    let canonical = if en {
        format!("https://lavunio.pro{path}?lang=en")
    } else {
        format!("https://lavunio.pro{path}")
    };
    let ru_url = format!("https://lavunio.pro{path}");
    let en_url = format!("https://lavunio.pro{path}?lang=en");

    format!(
        r##"<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="index,follow">
<meta name="theme-color" content="#080808">
<title>{title} — Lavunio</title>
<meta name="description" content="{lead}">
<link rel="icon" type="image/png" href="/legal/logo.png">
<meta property="og:type" content="article">
<meta property="og:title" content="{title} — Lavunio">
<meta property="og:description" content="{lead}">
<meta property="og:url" content="{canonical}">
<meta property="og:site_name" content="Lavunio">
<link rel="canonical" href="{canonical}">
<link rel="alternate" hreflang="ru" href="{ru_url}">
<link rel="alternate" hreflang="en" href="{en_url}">
<link rel="alternate" hreflang="x-default" href="{ru_url}">
<style>{style}</style>
</head>
<body>
<a class="skip" href="#content">{skip}</a>
<header class="topbar">
  <a class="brand" href="https://lavunio.pro" aria-label="Lavunio">
    <span class="brand-mark" aria-hidden="true">
      <img src="/legal/logo.png" alt="" width="32" height="32">
    </span>
    <span class="brand-name">LAVUNIO</span>
  </a>
  <nav class="topnav" aria-label="{nav_label}">
    <a class="nav-link{active_terms}" href="/legal/terms?lang={lang_query}">{terms}</a>
    <a class="nav-link{active_privacy}" href="/legal/privacy?lang={lang_query}">{privacy}</a>
    <a class="nav-link{active_rules}" href="/legal/rules?lang={lang_query}">{rules}</a>
  </nav>
  <div class="lang" aria-label="{lang_label}">
    <a class="lang-link{ru_active}" href="?lang=ru" aria-current="{ru_current}">RU</a>
    <a class="lang-link{en_active}" href="?lang=en" aria-current="{en_current}">EN</a>
  </div>
</header>

<main class="wrap">
  <section class="hero">
    <div class="eyebrow">Lavunio Legal</div>
    <h1>{title}</h1>
    <p class="lead">{lead}</p>
    <div class="meta">
      <span>{effective}</span>
      <span>{version}</span>
      <span><a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a></span>
    </div>
  </section>

  <div class="layout">
    <aside class="toc">
      <div class="toc-title">{toc_title}</div>
      <nav aria-label="{toc_title}">
        {toc}
      </nav>
      <div class="toc-contact">
        <strong>{support}</strong><br>
        <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>
      </div>
    </aside>

    <article class="doc" id="content">
      {body}
    </article>
  </div>
</main>

<footer>
  <div class="wrap foot">
    <div>© 2026 Lavunio. {rights}</div>
    <div class="foot-links">
      <a href="https://lavunio.pro">{home}</a>
      <a href="/legal/terms?lang={lang_query}">{terms}</a>
      <a href="/legal/privacy?lang={lang_query}">{privacy}</a>
      <a href="/legal/rules?lang={lang_query}">{rules}</a>
      <a href="mailto:lavunio.support@gmail.com">{support}</a>
    </div>
  </div>
  <div class="wrap foot-note">{footer_note}</div>
</footer>
</body>
</html>"##,
        lang = lang,
        title = title,
        lead = lead,
        style = STYLE,
        active_terms = active_terms,
        active_privacy = active_privacy,
        active_rules = active_rules,
        ru_active = ru_active,
        en_active = en_active,
        ru_current = if en { "false" } else { "true" },
        en_current = if en { "true" } else { "false" },
        lang_query = lang_query,
        terms = terms,
        privacy = privacy,
        rules = rules,
        toc_title = toc_title,
        toc = toc,
        support = support,
        effective = effective,
        version = version,
        home = home,
        rights = rights,
        footer_note = footer_note,
        skip = skip,
        nav_label = nav_label,
        lang_label = lang_label,
        canonical = canonical,
        ru_url = ru_url,
        en_url = en_url,
        body = body
    )
}

const STYLE: &str = r##"
:root {
  --bg: #080808;
  --panel: #0f0f0f;
  --panel-2: #141414;
  --text: #f4efe9;
  --body: #d8d1ca;
  --muted: #a69d95;
  --muted-2: #837a72;
  --line: rgba(255, 255, 255, 0.09);
  --line-soft: rgba(255, 255, 255, 0.055);
  --accent: #e5322b;
  --accent-2: #ff7a1f;
  --radius: 10px;
}
* { box-sizing: border-box; }
html { scroll-behavior: smooth; }
body {
  margin: 0;
  background:
    radial-gradient(900px 520px at 88% -12%, rgba(229, 50, 43, 0.14), transparent 62%),
    radial-gradient(760px 460px at -8% 8%, rgba(255, 122, 31, 0.07), transparent 58%),
    var(--bg);
  color: var(--text);
  font: 15px/1.72 "Segoe UI", system-ui, -apple-system, "Helvetica Neue", Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
}
body::before {
  content: "";
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 0;
  opacity: 0.35;
  background-image: linear-gradient(rgba(255,255,255,0.018) 1px, transparent 1px);
  background-size: 100% 4px;
}
a { color: var(--accent-2); }
::selection { background: rgba(229, 50, 43, 0.45); color: #fff; }
:where(a, button, input, summary):focus-visible {
  outline: 2px solid #ffb45e;
  outline-offset: 3px;
  border-radius: 4px;
}
.skip {
  position: absolute;
  left: 16px;
  top: -60px;
  z-index: 100;
  padding: 10px 14px;
  border-radius: 6px;
  background: var(--text);
  color: #080808;
  font-weight: 700;
  text-decoration: none;
}
.skip:focus { top: 12px; }

.topbar {
  position: sticky;
  top: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 26px;
  padding: 13px 32px;
  background: rgba(8, 8, 8, 0.88);
  border-bottom: 1px solid var(--line);
  backdrop-filter: blur(16px);
}
.brand {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  color: var(--text);
  text-decoration: none;
  flex: 0 0 auto;
}
.brand-mark { display: inline-flex; filter: drop-shadow(0 0 14px rgba(255, 90, 30, 0.25)); }
.brand-mark img { width: 32px; height: 32px; display: block; object-fit: cover; border-radius: 8px; }
.brand-name {
  font-family: "Arial Black", "Segoe UI", sans-serif;
  font-size: 17px;
  font-weight: 900;
  letter-spacing: 2.6px;
}
.topnav { display: flex; gap: 3px; margin-left: auto; }
.nav-link {
  padding: 8px 12px;
  border: 1px solid transparent;
  border-radius: 6px;
  color: var(--muted);
  font-size: 10.5px;
  font-weight: 800;
  letter-spacing: 0.9px;
  text-transform: uppercase;
  text-decoration: none;
  white-space: nowrap;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
}
.nav-link:hover { color: var(--text); background: var(--panel-2); }
.nav-link.active {
  color: #fff;
  background: linear-gradient(135deg, rgba(229, 50, 43, 0.25), rgba(255, 122, 31, 0.08));
  border-color: rgba(229, 50, 43, 0.34);
}
.lang {
  display: flex;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--line);
  border-radius: 6px;
  flex: 0 0 auto;
}
.lang-link {
  padding: 5px 8px;
  border-radius: 4px;
  color: var(--muted);
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.6px;
  text-decoration: none;
}
.lang-link:hover { color: var(--text); }
.lang-link.active { background: var(--text); color: #080808; }

.wrap { width: 100%; max-width: 1180px; margin: 0 auto; padding: 0 32px; position: relative; z-index: 1; }
.hero { padding: 62px 0 30px; border-bottom: 1px solid var(--line-soft); }
.eyebrow {
  color: var(--accent-2);
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 3.2px;
  text-transform: uppercase;
}
h1 {
  margin: 14px 0 16px;
  font-family: "Arial Black", "Segoe UI", sans-serif;
  font-size: clamp(30px, 4.6vw, 58px);
  line-height: 1.03;
  letter-spacing: -0.6px;
  text-transform: uppercase;
  overflow-wrap: anywhere;
}
@media (max-width: 560px) {
  h1 { font-size: 30px; letter-spacing: -0.3px; }
}
.lead {
  max-width: 800px;
  margin: 0 0 20px;
  color: var(--muted);
  font-size: 17px;
  line-height: 1.65;
}
.meta { display: flex; flex-wrap: wrap; gap: 8px; }
.meta span {
  padding: 5px 9px;
  border: 1px solid var(--line);
  border-radius: 5px;
  color: var(--muted-2);
  font-size: 11px;
}
.meta a { color: var(--muted); text-decoration: none; }
.meta a:hover { color: var(--text); }

.layout {
  display: grid;
  grid-template-columns: 250px minmax(0, 1fr);
  gap: 48px;
  padding: 42px 0 74px;
}
.toc {
  position: sticky;
  top: 86px;
  align-self: start;
  padding: 18px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: linear-gradient(180deg, #101010, #0b0b0b);
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.28);
}
.toc-title {
  margin-bottom: 12px;
  color: var(--muted-2);
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 1.7px;
  text-transform: uppercase;
}
.toc nav { display: flex; flex-direction: column; gap: 1px; }
.toc a {
  padding: 6px 8px;
  border-left: 2px solid transparent;
  border-radius: 4px;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.4;
  text-decoration: none;
  transition: background 0.14s, color 0.14s, border-color 0.14s;
}
.toc a:hover { color: var(--text); background: var(--panel-2); border-left-color: var(--accent); }
.toc-contact {
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
  color: var(--muted-2);
  font-size: 11px;
  line-height: 1.6;
}
.toc-contact strong { color: var(--muted); }
.toc-contact a { color: var(--accent-2); text-decoration: none; }
.toc-contact a:hover { text-decoration: underline; }

.doc { min-width: 0; }
.doc section { padding-bottom: 26px; margin-bottom: 26px; border-bottom: 1px solid var(--line-soft); }
.doc section:last-child { padding-bottom: 0; margin-bottom: 0; border-bottom: 0; }
.doc h2 {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin: 0 0 12px;
  font-family: "Arial Black", "Segoe UI", sans-serif;
  font-size: 15.5px;
  line-height: 1.3;
  letter-spacing: 0.35px;
  text-transform: uppercase;
}
.doc h2::before {
  content: "";
  flex: 0 0 auto;
  width: 3px;
  height: 15px;
  border-radius: 2px;
  background: linear-gradient(180deg, var(--accent-2), var(--accent));
  box-shadow: 0 0 12px rgba(255, 90, 30, 0.35);
}
.doc p { margin: 0 0 12px; color: var(--body); }
.doc ul { margin: 0 0 12px; padding-left: 20px; }
.doc li { margin: 6px 0; color: var(--body); }
.doc li::marker { color: var(--accent-2); }
.doc strong { color: #fff; }
.doc a { color: #ff9a5c; text-decoration: none; border-bottom: 1px solid rgba(255, 154, 92, 0.3); }
.doc a:hover { color: #ffb47e; border-bottom-color: currentColor; }
.note {
  margin-top: 14px;
  padding: 12px 14px;
  border: 1px solid rgba(255, 122, 31, 0.22);
  border-radius: 8px;
  background: rgba(255, 122, 31, 0.055);
  color: var(--muted);
  font-size: 13px;
}

footer {
  position: relative;
  z-index: 1;
  padding: 24px 0 38px;
  background: #060606;
  border-top: 1px solid var(--line);
}
.foot {
  display: flex;
  flex-wrap: wrap;
  gap: 16px 28px;
  align-items: center;
  justify-content: space-between;
  color: var(--muted-2);
  font-size: 11.5px;
}
.foot-links { display: flex; flex-wrap: wrap; gap: 15px; }
.foot a { color: var(--muted); text-decoration: none; }
.foot a:hover { color: var(--text); }
.foot-note { margin-top: 12px; color: #5f5751; font-size: 11px; }

@media (max-width: 920px) {
  .topbar { flex-wrap: wrap; gap: 10px 14px; padding: 12px 18px; }
  .brand { margin-right: auto; }
  .topnav { order: 3; width: 100%; margin-left: 0; overflow-x: auto; padding-bottom: 2px; }
  .wrap { padding: 0 18px; }
  .hero { padding-top: 40px; }
  .layout { grid-template-columns: 1fr; gap: 26px; padding-top: 28px; }
  .toc { position: static; }
  .toc nav { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .doc { order: 2; }
}
@media (max-width: 560px) {
  .meta span { font-size: 10px; }
  .lead { font-size: 15.5px; }
  .toc nav { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .toc a { font-size: 11px; }
  .foot { flex-direction: column; align-items: flex-start; }
}
@media (max-width: 390px) {
  .toc nav { grid-template-columns: 1fr; }
}
@media (prefers-reduced-motion: reduce) {
  html { scroll-behavior: auto; }
  *, *::before, *::after { animation-duration: 0.001ms !important; transition-duration: 0.001ms !important; }
}
@media print {
  body { background: #fff; color: #111; }
  body::before, .topbar, .toc, footer, .skip { display: none; }
  .wrap { max-width: none; padding: 0; }
  .hero { border: 0; padding-top: 0; }
  .doc p, .doc li { color: #111; }
  .doc a { color: #111; }
}
"##;

const TERMS_TOC_RU: &str = r##"
<a href="#general">1. Общие положения</a>
<a href="#account">2. Регистрация и аккаунт</a>
<a href="#content">3. Пользовательский контент</a>
<a href="#authors">4. Контент авторов</a>
<a href="#prohibited">5. Запрещённые действия</a>
<a href="#moderation">6. Модерация и санкции</a>
<a href="#ip">7. Интеллектуальная собственность</a>
<a href="#availability">8. Доступность сервиса</a>
<a href="#liability">9. Ограничение ответственности</a>
<a href="#termination">10. Прекращение доступа</a>
<a href="#changes">11. Изменения соглашения</a>
<a href="#contact">12. Контакты</a>
"##;

const TERMS_RU: &str = r##"
<section id="general">
<h2>1. Общие положения</h2>
<p>Lavunio — платформа для публикации, поиска и установки игровых режимов Minecraft. Соглашение распространяется на лаунчер, сайт <a href="https://lavunio.pro">lavunio.pro</a>, каталог, API и связанные сервисы.</p>
<ul>
<li>Используя Lavunio, вы принимаете условия Соглашения. Если вы не согласны с ними, прекратите использование сервиса.</li>
<li>Соглашение может обновляться. Новая редакция публикуется на этой странице и вступает в силу с указанной даты.</li>
<li>Продолжение использования сервиса после вступления изменений в силу означает согласие с новой редакцией.</li>
</ul>
</section>
<section id="account">
<h2>2. Регистрация и аккаунт</h2>
<ul>
<li>Для публикации режимов, друзей, чата и других социальных функций требуется аккаунт Lavunio.</li>
<li>Указывайте достоверные данные и не передавайте пароль третьим лицам. Вы отвечаете за действия, совершённые под вашим аккаунтом.</li>
<li>Если вам меньше 18 лет, используйте сервис с согласия законного представителя.</li>
<li>Мы можем запросить подтверждение почты и ограничить аккаунты, созданные с недостоверной информацией.</li>
<li>При подозрении на несанкционированный доступ смените пароль и свяжитесь с поддержкой.</li>
</ul>
</section>
<section id="content">
<h2>3. Пользовательский контент</h2>
<ul>
<li>Вы сохраняете права на свои материалы. Загружая контент, вы предоставляете Lavunio неисключительную, всемирную и безвозмездную лицензию на хранение, отображение и распространение этого контента в рамках платформы.</li>
<li>Вы подтверждаете, что обладаете правами на сборки, моды, изображения, тексты и другие материалы, которые загружаете.</li>
<li>Запрещено загружать пиратские сборки, материалы без разрешения правообладателя и контент, нарушающий законодательство.</li>
<li>Мы можем отклонить, скрыть или удалить контент, нарушающий Соглашение, правила сообщества или закон.</li>
<li>После удаления контента его копии могут некоторое время сохраняться в резервных копиях.</li>
</ul>
</section>
<section id="authors">
<h2>4. Контент авторов и релизы</h2>
<ul>
<li>Автор публикует сборку на свой риск и обязан честно указывать версию, состав, требования и известные ограничения.</li>
<li>Опубликованные релизы неизменяемы: файлы версии нельзя перезаписать. Для исправлений создаётся новый релиз.</li>
<li>Мы проверяем архивы на безопасность, но не гарантируем полное отсутствие вредоносного кода в сторонних сборках. Устанавливайте контент осознанно.</li>
<li>Автор обязан поддерживать свои релизы и отвечать на обращения модераторов.</li>
<li>Запрещены скрытые исполняемые файлы, обфускация без объяснения и любые попытки обойти проверки платформы.</li>
</ul>
</section>
<section id="prohibited">
<h2>5. Запрещённые действия</h2>
<ul>
<li>Вредоносный код, вирусы, стилеры, майнеры, эксплойты и обход защиты.</li>
<li>Незаконный контент, пиратские материалы и нарушение прав интеллектуальной собственности.</li>
<li>Оскорбления, угрозы, травля, разжигание вражды и дискриминация.</li>
<li>Спам, мошенничество, накрутка рейтингов и выдача себя за другого автора, команду или сотрудника Lavunio.</li>
<li>Попытки получить доступ к чужим аккаунтам, данным или инфраструктуре, а также чрезмерная нагрузка на сервис.</li>
<li>Публикация чужих персональных данных без согласия.</li>
</ul>
</section>
<section id="moderation">
<h2>6. Модерация и санкции</h2>
<ul>
<li>Модераторы могут отклонить релиз, скрыть режим, удалить контент или ограничить аккаунт.</li>
<li>Санкции зависят от характера нарушения и его повторяемости: предупреждение, удаление контента, временное ограничение или блокировка.</li>
<li>Решение модератора можно оспорить через раздел «Публикация» в лаунчере или по почте поддержки.</li>
<li>Апелляция рассматривается повторно; решение по ней фиксируется в журнале модерации.</li>
</ul>
</section>
<section id="ip">
<h2>7. Интеллектуальная собственность</h2>
<p>Название Lavunio, логотип, дизайн, интерфейс, код и другие материалы платформы принадлежат Lavunio или используются на законных основаниях. Их нельзя копировать, изменять или распространять без разрешения, кроме случаев, предусмотренных законом.</p>
</section>
<section id="availability">
<h2>8. Доступность сервиса</h2>
<ul>
<li>Сервис предоставляется «как есть» и «по мере доступности».</li>
<li>Мы можем изменять, приостанавливать или прекращать работу отдельных функций, включая каталог, публикацию и социальные разделы.</li>
<li>Мы не гарантируем бесперебойную работу, совместимость со всеми компьютерами, версиями Java и сторонними модами.</li>
<li>Плановые работы и обновления могут временно ограничивать доступ.</li>
</ul>
</section>
<section id="liability">
<h2>9. Ограничение ответственности</h2>
<ul>
<li>В пределах, допускаемых законом, Lavunio не несёт ответственности за косвенные убытки, потерю данных, упущенную выгоду и последствия использования сторонних сборок.</li>
<li>Вы самостоятельно оцениваете риски установки пользовательского контента и делаете резервные копии важных данных.</li>
<li>Ничто в Соглашении не ограничивает права потребителя и ответственность, которая не может быть ограничена по закону.</li>
</ul>
</section>
<section id="termination">
<h2>10. Прекращение доступа</h2>
<ul>
<li>Вы можете прекратить использование сервиса в любое время и запросить удаление аккаунта через поддержку.</li>
<li>Мы можем приостановить или заблокировать аккаунт за нарушение Соглашения, правил сообщества или законодательства.</li>
<li>Положения об интеллектуальной собственности и ограничении ответственности сохраняют силу после прекращения доступа.</li>
</ul>
</section>
<section id="changes">
<h2>11. Изменения соглашения</h2>
<p>Мы можем обновлять Соглашение. Новая редакция публикуется на этой странице. О существенных изменениях мы постараемся сообщить в лаунчере или по электронной почте.</p>
</section>
<section id="contact">
<h2>12. Контакты</h2>
<p>Вопросы по Соглашению: <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
<div class="note">Перед использованием сервиса ознакомьтесь также с <a href="/legal/privacy">Политикой конфиденциальности</a> и <a href="/legal/rules">Правилами сообщества</a>.</div>
</section>
"##;

const TERMS_TOC_EN: &str = r##"
<a href="#general">1. General</a>
<a href="#account">2. Registration and account</a>
<a href="#content">3. User content</a>
<a href="#authors">4. Author content</a>
<a href="#prohibited">5. Prohibited actions</a>
<a href="#moderation">6. Moderation and sanctions</a>
<a href="#ip">7. Intellectual property</a>
<a href="#availability">8. Service availability</a>
<a href="#liability">9. Limitation of liability</a>
<a href="#termination">10. Termination</a>
<a href="#changes">11. Changes to the terms</a>
<a href="#contact">12. Contact</a>
"##;

const TERMS_EN: &str = r##"
<section id="general">
<h2>1. General</h2>
<p>Lavunio is a platform for publishing, discovering and installing Minecraft game modes. These terms apply to the launcher, the <a href="https://lavunio.pro">lavunio.pro</a> website, the catalog, the API and related services.</p>
<ul>
<li>By using Lavunio you accept these terms. If you do not agree, stop using the service.</li>
<li>We may update these terms. The new version is published on this page and takes effect on the stated date.</li>
<li>Continued use after the effective date means acceptance of the updated terms.</li>
</ul>
</section>
<section id="account">
<h2>2. Registration and account</h2>
<ul>
<li>A Lavunio account is required for publishing, friends, chat and other social features.</li>
<li>Provide accurate information and do not share your password. You are responsible for activity under your account.</li>
<li>If you are under 18, use the service with the consent of your legal representative.</li>
<li>We may require email verification and restrict accounts created with false information.</li>
<li>If you suspect unauthorized access, change your password and contact support.</li>
</ul>
</section>
<section id="content">
<h2>3. User content</h2>
<ul>
<li>You retain the rights to your materials. By uploading content, you grant Lavunio a non-exclusive, worldwide, royalty-free license to store, display and distribute it within the platform.</li>
<li>You confirm that you hold the rights to the builds, mods, images, texts and other materials you upload.</li>
<li>Pirated builds, unlicensed materials and content that violates the law are prohibited.</li>
<li>We may reject, hide or remove content that violates these terms, the community rules or the law.</li>
<li>After deletion, copies may remain in backups for a limited time.</li>
</ul>
</section>
<section id="authors">
<h2>4. Author content and releases</h2>
<ul>
<li>Authors publish builds at their own risk and must accurately describe the version, contents, requirements and known limitations.</li>
<li>Published releases are immutable: files for a version cannot be overwritten. Fixes require a new release.</li>
<li>We scan archives for security, but we do not guarantee that third-party builds are completely free of malicious code. Install content responsibly.</li>
<li>Authors must maintain their releases and respond to moderator requests.</li>
<li>Hidden executables, unexplained obfuscation and attempts to bypass platform checks are prohibited.</li>
</ul>
</section>
<section id="prohibited">
<h2>5. Prohibited actions</h2>
<ul>
<li>Malware, viruses, stealers, miners, exploits and protection bypasses.</li>
<li>Illegal content, pirated materials and intellectual property infringement.</li>
<li>Harassment, threats, hate speech, discrimination and abuse.</li>
<li>Spam, fraud, rating manipulation and impersonation of another author, team or Lavunio staff member.</li>
<li>Attempts to access other accounts, data or infrastructure, or to overload the service.</li>
<li>Publishing personal data of others without consent.</li>
</ul>
</section>
<section id="moderation">
<h2>6. Moderation and sanctions</h2>
<ul>
<li>Moderators may reject a release, hide a mode, remove content or restrict an account.</li>
<li>Sanctions depend on the nature and repetition of the violation: warning, content removal, temporary restriction or suspension.</li>
<li>Moderation decisions can be appealed through the Publisher section in the launcher or by email.</li>
<li>Appeals are reviewed again and the outcome is recorded in the moderation log.</li>
</ul>
</section>
<section id="ip">
<h2>7. Intellectual property</h2>
<p>The Lavunio name, logo, design, interface, code and other platform materials belong to Lavunio or are used lawfully. They may not be copied, modified or distributed without permission, except as permitted by law.</p>
</section>
<section id="availability">
<h2>8. Service availability</h2>
<ul>
<li>The service is provided “as is” and “as available”.</li>
<li>We may change, suspend or discontinue individual features, including the catalog, publishing and social sections.</li>
<li>We do not guarantee uninterrupted operation or compatibility with every computer, Java version or third-party mod.</li>
<li>Maintenance and updates may temporarily limit access.</li>
</ul>
</section>
<section id="liability">
<h2>9. Limitation of liability</h2>
<ul>
<li>To the extent permitted by law, Lavunio is not liable for indirect damages, data loss, lost profits or the consequences of using third-party builds.</li>
<li>You assess the risks of installing user content yourself and keep backups of important data.</li>
<li>Nothing in these terms limits consumer rights or liability that cannot be limited by law.</li>
</ul>
</section>
<section id="termination">
<h2>10. Termination</h2>
<ul>
<li>You may stop using the service at any time and request account deletion through support.</li>
<li>We may suspend or block an account for violating these terms, the community rules or the law.</li>
<li>Provisions on intellectual property and limitation of liability survive termination.</li>
</ul>
</section>
<section id="changes">
<h2>11. Changes to the terms</h2>
<p>We may update these terms. The new version is published on this page. We will try to announce material changes in the launcher or by email.</p>
</section>
<section id="contact">
<h2>12. Contact</h2>
<p>Questions about these terms: <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
<div class="note">Please also read the <a href="/legal/privacy?lang=en">Privacy Policy</a> and the <a href="/legal/rules?lang=en">Community Rules</a> before using the service.</div>
</section>
"##;

const PRIVACY_TOC_RU: &str = r##"
<a href="#controller">1. Кто обрабатывает данные</a>
<a href="#data">2. Какие данные мы собираем</a>
<a href="#bases">3. Правовые основания</a>
<a href="#purposes">4. Зачем используем данные</a>
<a href="#storage">5. Где хранятся данные</a>
<a href="#retention">6. Срок хранения</a>
<a href="#sharing">7. Передача данных</a>
<a href="#cookies">8. Cookies и локальное хранилище</a>
<a href="#security">9. Безопасность</a>
<a href="#rights">10. Ваши права</a>
<a href="#children">11. Дети</a>
<a href="#changes">12. Изменения политики</a>
<a href="#contact">13. Контакты</a>
"##;

const PRIVACY_RU: &str = r##"
<section id="controller">
<h2>1. Кто обрабатывает данные</h2>
<p>Оператором данных, связанных с работой Lavunio, является команда Lavunio. По вопросам обработки данных и реализации ваших прав пишите на <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
</section>
<section id="data">
<h2>2. Какие данные мы собираем</h2>
<ul>
<li><strong>Аккаунт:</strong> email, username, отображаемое имя, хеш пароля, роль, статус подтверждения почты.</li>
<li><strong>Профиль:</strong> аватар, описание, Minecraft UUID, скины и плащи.</li>
<li><strong>Безопасность:</strong> метаданные сессий, IP-адрес, user-agent, время входа и служебные идентификаторы.</li>
<li><strong>Социальные функции:</strong> друзья, заявки, личные сообщения, членство в командах, уведомления.</li>
<li><strong>Публикация:</strong> метаданные режимов и релизов, сборки, обложки, скриншоты и результаты проверки архивов.</li>
<li><strong>Технические данные:</strong> логи API, ошибки, события rate limit и сведения о стабильности сервиса.</li>
</ul>
<p>Мы не запрашиваем специальные категории персональных данных и не обрабатываем платёжные данные, поскольку монетизация пока не подключена.</p>
</section>
<section id="bases">
<h2>3. Правовые основания</h2>
<ul>
<li>Исполнение договора — предоставление аккаунта, каталога, библиотеки, публикации и социальных функций.</li>
<li>Согласие — необязательные уведомления и отдельные функции, для которых требуется явное согласие.</li>
<li>Законные интересы — безопасность, предотвращение злоупотреблений, модерация и улучшение стабильности.</li>
<li>Требования закона — хранение сведений, необходимых для соблюдения правовых обязанностей.</li>
</ul>
</section>
<section id="purposes">
<h2>4. Зачем используем данные</h2>
<ul>
<li>Регистрация, вход, восстановление доступа и подтверждение почты.</li>
<li>Профиль, скины, библиотека, список желаемого и подписки.</li>
<li>Публикация режимов, проверка сборок, модерация и апелляции.</li>
<li>Друзья, присутствие, чат, уведомления и лента.</li>
<li>Поддержка пользователей и ответы на обращения.</li>
<li>Защита от спама, мошенничества и атак, диагностика ошибок и повышение стабильности.</li>
</ul>
</section>
<section id="storage">
<h2>5. Где хранятся данные</h2>
<p>Основная база данных PostgreSQL размещена на VPS-сервере. Сборки, обложки, скриншоты и скины хранятся в S3-совместимом объектном хранилище. Письма отправляются через Resend. Мы не продаём и не сдаём данные в аренду третьим лицам.</p>
<p>Поставщики инфраструктуры могут обрабатывать данные в других юрисдикциях. Мы выбираем поставщиков с разумными мерами защиты и ограничиваем доступ к данным.</p>
</section>
<section id="retention">
<h2>6. Срок хранения</h2>
<ul>
<li>Данные аккаунта хранятся до удаления аккаунта или до прекращения использования сервиса.</li>
<li>Резервные копии базы хранятся до 7 дней.</li>
<li>Технические логи хранятся ограниченное время, необходимое для диагностики и безопасности.</li>
<li>Записи модерации могут храниться дольше для защиты платформы и разбора повторных нарушений.</li>
<li>После удаления контента его копии могут оставаться в резервных копиях до их ротации.</li>
</ul>
</section>
<section id="sharing">
<h2>7. Передача данных</h2>
<p>Мы передаём данные только в объёме, необходимом для работы сервиса: поставщикам хостинга, объектного хранилища и доставки почты. Данные также могут быть раскрыты, если это требуется законом, необходимо для защиты прав и безопасности Lavunio или пользователей, либо осуществляется с вашего согласия.</p>
</section>
<section id="cookies">
<h2>8. Cookies и локальное хранилище</h2>
<p>Лаунчер хранит настройки и сессию локально на вашем компьютере. Юридические страницы не используют рекламные cookies, сторонние трекеры и аналитику. Язык страницы выбирается через параметр запроса и не требует cookies.</p>
</section>
<section id="security">
<h2>9. Безопасность</h2>
<ul>
<li>Пароли хранятся в виде хешей Argon2id.</li>
<li>Передача данных идёт по HTTPS, доступ к серверу ограничен.</li>
<li>Действуют rate limit, журналирование и резервное копирование.</li>
<li>Ни одна система не гарантирует абсолютную безопасность. Если вы заметили уязвимость, сообщите нам по почте поддержки.</li>
</ul>
</section>
<section id="rights">
<h2>10. Ваши права</h2>
<p>В зависимости от применимого законодательства вы можете запросить доступ к своим данным, их исправление, удаление, ограничение обработки, перенос данных, отзыв согласия или возразить против обработки. Напишите на <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>; мы ответим в срок, установленный законом. Вы также вправе обратиться в надзорный орган.</p>
</section>
<section id="children">
<h2>11. Дети</h2>
<p>Сервис не предназначен для детей младше 13 лет. Пользователи младше 18 лет должны использовать Lavunio с согласия законного представителя. Если мы узнаем, что данные ребёнка были получены без необходимого согласия, мы примем меры для их удаления.</p>
</section>
<section id="changes">
<h2>12. Изменения политики</h2>
<p>Новая редакция политики публикуется на этой странице. О существенных изменениях мы постараемся сообщить в лаунчере или по электронной почте.</p>
</section>
<section id="contact">
<h2>13. Контакты</h2>
<p>Вопросы о конфиденциальности и запросы по данным: <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
<div class="note">Перед использованием сервиса ознакомьтесь также с <a href="/legal/terms">Пользовательским соглашением</a> и <a href="/legal/rules">Правилами сообщества</a>.</div>
</section>
"##;

const PRIVACY_TOC_EN: &str = r##"
<a href="#controller">1. Data controller</a>
<a href="#data">2. Data we collect</a>
<a href="#bases">3. Legal bases</a>
<a href="#purposes">4. How we use data</a>
<a href="#storage">5. Where data is stored</a>
<a href="#retention">6. Retention</a>
<a href="#sharing">7. Data sharing</a>
<a href="#cookies">8. Cookies and local storage</a>
<a href="#security">9. Security</a>
<a href="#rights">10. Your rights</a>
<a href="#children">11. Children</a>
<a href="#changes">12. Policy changes</a>
<a href="#contact">13. Contact</a>
"##;

const PRIVACY_EN: &str = r##"
<section id="controller">
<h2>1. Data controller</h2>
<p>The Lavunio team is the controller of data processed through Lavunio. For privacy questions and data rights requests, contact <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
</section>
<section id="data">
<h2>2. Data we collect</h2>
<ul>
<li><strong>Account:</strong> email, username, display name, password hash, role and email verification status.</li>
<li><strong>Profile:</strong> avatar, bio, Minecraft UUID, skins and capes.</li>
<li><strong>Security:</strong> session metadata, IP address, user agent, sign-in timestamps and service identifiers.</li>
<li><strong>Social features:</strong> friends, requests, direct messages, team membership and notifications.</li>
<li><strong>Publishing:</strong> mode and release metadata, builds, covers, screenshots and archive scan results.</li>
<li><strong>Technical data:</strong> API logs, errors, rate-limit events and service stability information.</li>
</ul>
<p>We do not request special categories of personal data and do not process payment data because monetization is not enabled yet.</p>
</section>
<section id="bases">
<h2>3. Legal bases</h2>
<ul>
<li>Performance of a contract — providing the account, catalog, library, publishing and social features.</li>
<li>Consent — optional notifications and features that require explicit consent.</li>
<li>Legitimate interests — security, abuse prevention, moderation and service stability.</li>
<li>Legal obligations — retaining information required to comply with applicable law.</li>
</ul>
</section>
<section id="purposes">
<h2>4. How we use data</h2>
<ul>
<li>Registration, sign-in, account recovery and email verification.</li>
<li>Profiles, skins, library, wishlist and follows.</li>
<li>Mode publishing, build checks, moderation and appeals.</li>
<li>Friends, presence, chat, notifications and the activity feed.</li>
<li>User support and responses to requests.</li>
<li>Protection against spam, fraud and attacks, error diagnostics and stability improvements.</li>
</ul>
</section>
<section id="storage">
<h2>5. Where data is stored</h2>
<p>The main PostgreSQL database is hosted on a VPS server. Builds, covers, screenshots and skins are stored in S3-compatible object storage. Email is delivered through Resend. We do not sell or rent data to third parties.</p>
<p>Infrastructure providers may process data in other jurisdictions. We select providers with reasonable safeguards and restrict access to data.</p>
</section>
<section id="retention">
<h2>6. Retention</h2>
<ul>
<li>Account data is kept until account deletion or until you stop using the service.</li>
<li>Database backups are kept for up to 7 days.</li>
<li>Technical logs are retained for the limited time needed for diagnostics and security.</li>
<li>Moderation records may be retained longer to protect the platform and review repeat violations.</li>
<li>After content deletion, copies may remain in backups until they are rotated.</li>
</ul>
</section>
<section id="sharing">
<h2>7. Data sharing</h2>
<p>We share data only to the extent necessary to operate the service: with hosting, object storage and email delivery providers. Data may also be disclosed where required by law, necessary to protect the rights and safety of Lavunio or users, or with your consent.</p>
</section>
<section id="cookies">
<h2>8. Cookies and local storage</h2>
<p>The launcher stores settings and the session locally on your computer. Legal pages use no advertising cookies, third-party trackers or analytics. The page language is selected by a query parameter and does not require cookies.</p>
</section>
<section id="security">
<h2>9. Security</h2>
<ul>
<li>Passwords are stored as Argon2id hashes.</li>
<li>Data is transmitted over HTTPS and server access is restricted.</li>
<li>Rate limits, logging and backups are in place.</li>
<li>No system is completely secure. If you discover a vulnerability, report it to support.</li>
</ul>
</section>
<section id="rights">
<h2>10. Your rights</h2>
<p>Depending on applicable law, you may request access to your data, correction, deletion, restriction of processing, data portability, withdrawal of consent or object to processing. Write to <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>; we will respond within the period required by law. You may also contact a supervisory authority.</p>
</section>
<section id="children">
<h2>11. Children</h2>
<p>The service is not intended for children under 13. Users under 18 must use Lavunio with the consent of a legal representative. If we learn that a child's data was collected without the required consent, we will take steps to delete it.</p>
</section>
<section id="changes">
<h2>12. Policy changes</h2>
<p>A new version of this policy is published on this page. We will try to announce material changes in the launcher or by email.</p>
</section>
<section id="contact">
<h2>13. Contact</h2>
<p>Privacy questions and data requests: <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
<div class="note">Please also read the <a href="/legal/terms?lang=en">Terms of Use</a> and the <a href="/legal/rules?lang=en">Community Rules</a> before using the service.</div>
</section>
"##;

const RULES_TOC_RU: &str = r##"
<a href="#principles">1. Общие принципы</a>
<a href="#prohibited">2. Запрещённый контент</a>
<a href="#authors">3. Правила авторов</a>
<a href="#communication">4. Правила общения</a>
<a href="#moderation">5. Публикация и модерация</a>
<a href="#sanctions">6. Санкции</a>
<a href="#appeals">7. Апелляции</a>
<a href="#reports">8. Жалобы</a>
<a href="#contact">9. Контакты</a>
"##;

const RULES_RU: &str = r##"
<section id="principles">
<h2>1. Общие принципы</h2>
<p>Мы хотим, чтобы Lavunio оставался безопасным и удобным местом для игроков, авторов и команд. Уважайте других пользователей, публикуйте честный контент и не создавайте угрозу для чужих устройств и данных.</p>
</section>
<section id="prohibited">
<h2>2. Запрещённый контент</h2>
<ul>
<li>Вредоносные файлы, вирусы, стилеры, майнеры, эксплойты и обход защиты.</li>
<li>Пиратские сборки, моды и материалы без разрешения правообладателя.</li>
<li>Незаконный контент, пропаганда насилия и дискриминация.</li>
<li>Оскорбления, угрозы, травля, разжигание вражды и публикация чужих персональных данных.</li>
<li>Спам, мошенничество, накрутка рейтингов, отзывов и установок.</li>
<li>Выдача себя за другого автора, команду или сотрудника Lavunio.</li>
<li>Вводящие в заблуждение описание, версия, состав сборки или скриншоты.</li>
</ul>
</section>
<section id="authors">
<h2>3. Правила авторов</h2>
<ul>
<li>Указывайте точные название, версию, ядро, требования и состав сборки.</li>
<li>Загружайте только те материалы, на которые у вас есть права.</li>
<li>Не добавляйте скрытые исполняемые файлы, обфусцированный код без объяснения и подозрительные компоненты.</li>
<li>Проверяйте сборку перед публикацией и поддерживайте релизы после выпуска.</li>
<li>Не накручивайте скачивания, установки, рейтинги и отзывы, в том числе через несколько аккаунтов.</li>
<li>Отвечайте на запросы модераторов и исправляйте нарушения в разумный срок.</li>
</ul>
</section>
<section id="communication">
<h2>4. Правила общения</h2>
<ul>
<li>Общайтесь уважительно и не переходите на личности.</li>
<li>Не публикуйте личные данные других людей без их согласия.</li>
<li>Не спамьте, не рекламируйте сторонние сервисы без разрешения и не используйте чат для мошенничества.</li>
<li>Сообщайте о нарушениях через кнопку «Пожаловаться» или по почте поддержки, а не устраивайте конфликты.</li>
</ul>
</section>
<section id="moderation">
<h2>5. Публикация и модерация</h2>
<ul>
<li>Релизы и другой контент могут проходить проверку перед публикацией.</li>
<li>Модератор может отклонить релиз с указанием причины, скрыть режим, удалить контент или запросить исправления.</li>
<li>Архивы проверяются на path traversal, опасные файлы и подозрительные компоненты. Пометка модератора не является гарантией полной безопасности.</li>
<li>Решение модератора фиксируется в журнале действий.</li>
</ul>
</section>
<section id="sanctions">
<h2>6. Санкции</h2>
<ul>
<li>Предупреждение и требование исправить нарушение.</li>
<li>Удаление или скрытие контента.</li>
<li>Временное ограничение публикации, чата или социальных функций.</li>
<li>Приостановка или блокировка аккаунта при повторных или грубых нарушениях.</li>
</ul>
<p>Санкции применяются пропорционально нарушению и его повторяемости.</p>
</section>
<section id="appeals">
<h2>7. Апелляции</h2>
<p>Отклонённый релиз можно оспорить в разделе «Публикация» в лаунчере. Опишите, что было исправлено или почему решение кажется ошибочным. Апелляция рассматривается модератором повторно, результат фиксируется в журнале.</p>
</section>
<section id="reports">
<h2>8. Жалобы</h2>
<p>Сообщить о нарушении можно кнопкой «Пожаловаться» на странице режима или письмом на <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>. Приложите ссылки, скриншоты и описание ситуации. Заведомо ложные жалобы могут привести к санкциям.</p>
</section>
<section id="contact">
<h2>9. Контакты</h2>
<p>Вопросы по правилам и жалобы: <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
<div class="note">Правила дополняют <a href="/legal/terms">Пользовательское соглашение</a> и <a href="/legal/privacy">Политику конфиденциальности</a>.</div>
</section>
"##;

const RULES_TOC_EN: &str = r##"
<a href="#principles">1. Principles</a>
<a href="#prohibited">2. Prohibited content</a>
<a href="#authors">3. Author rules</a>
<a href="#communication">4. Communication</a>
<a href="#moderation">5. Publishing and moderation</a>
<a href="#sanctions">6. Sanctions</a>
<a href="#appeals">7. Appeals</a>
<a href="#reports">8. Reports</a>
<a href="#contact">9. Contact</a>
"##;

const RULES_EN: &str = r##"
<section id="principles">
<h2>1. Principles</h2>
<p>We want Lavunio to remain a safe and useful place for players, authors and teams. Respect other users, publish honest content and do not endanger other people's devices or data.</p>
</section>
<section id="prohibited">
<h2>2. Prohibited content</h2>
<ul>
<li>Malware, viruses, stealers, miners, exploits and protection bypasses.</li>
<li>Pirated builds, mods and unlicensed materials.</li>
<li>Illegal content, promotion of violence and discrimination.</li>
<li>Harassment, threats, hate speech and publishing personal data of others.</li>
<li>Spam, fraud and manipulation of ratings, reviews or installs.</li>
<li>Impersonation of another author, team or Lavunio staff member.</li>
<li>Misleading descriptions, versions, build contents or screenshots.</li>
</ul>
</section>
<section id="authors">
<h2>3. Author rules</h2>
<ul>
<li>Provide accurate titles, versions, loaders, requirements and build contents.</li>
<li>Upload only materials you have the rights to use.</li>
<li>Do not include hidden executables, unexplained obfuscated code or suspicious components.</li>
<li>Test builds before publishing and maintain releases after launch.</li>
<li>Do not manipulate downloads, installs, ratings or reviews, including through multiple accounts.</li>
<li>Respond to moderator requests and fix violations within a reasonable time.</li>
</ul>
</section>
<section id="communication">
<h2>4. Communication</h2>
<ul>
<li>Communicate respectfully and avoid personal attacks.</li>
<li>Do not publish other people's personal data without consent.</li>
<li>Do not spam, advertise third-party services without permission or use chat for fraud.</li>
<li>Report violations with the Report button or by email instead of starting conflicts.</li>
</ul>
</section>
<section id="moderation">
<h2>5. Publishing and moderation</h2>
<ul>
<li>Releases and other content may be reviewed before publication.</li>
<li>Moderators may reject a release with a reason, hide a mode, remove content or request fixes.</li>
<li>Archives are checked for path traversal, dangerous files and suspicious components. A moderator review is not a guarantee of complete safety.</li>
<li>Moderation decisions are recorded in the action log.</li>
</ul>
</section>
<section id="sanctions">
<h2>6. Sanctions</h2>
<ul>
<li>A warning and a request to fix the violation.</li>
<li>Removal or hiding of content.</li>
<li>Temporary restriction of publishing, chat or social features.</li>
<li>Suspension or blocking of the account for repeated or serious violations.</li>
</ul>
<p>Sanctions are applied in proportion to the violation and its repetition.</p>
</section>
<section id="appeals">
<h2>7. Appeals</h2>
<p>A rejected release can be appealed in the Publisher section of the launcher. Describe what was fixed or why the decision appears incorrect. A moderator reviews the appeal again and the outcome is recorded in the log.</p>
</section>
<section id="reports">
<h2>8. Reports</h2>
<p>Report violations with the Report button on a mode page or by email at <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>. Include links, screenshots and a description. Deliberately false reports may lead to sanctions.</p>
</section>
<section id="contact">
<h2>9. Contact</h2>
<p>Rules questions and reports: <a href="mailto:lavunio.support@gmail.com">lavunio.support@gmail.com</a>.</p>
<div class="note">These rules supplement the <a href="/legal/terms?lang=en">Terms of Use</a> and the <a href="/legal/privacy?lang=en">Privacy Policy</a>.</div>
</section>
"##;
