use std::time::Duration;

use serde_json::json;

use crate::config::Config;

#[derive(Clone)]
pub struct Mailer {
    client: reqwest::Client,
    api_key: Option<String>,
    from: String,
    reply_to: Option<String>,
    base_url: String,
}

impl Mailer {
    pub fn new(config: &Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            api_key: config.resend_api_key.clone(),
            from: config.mail_from.clone(),
            reply_to: config.mail_reply_to.clone(),
            base_url: config.public_base_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn enabled(&self) -> bool {
        self.api_key.is_some()
    }

    pub async fn send_verification_code(&self, to: &str, code: &str) -> Result<(), String> {
        let subject = "Код подтверждения Lavunio";
        let email = escape_html(to);
        let html = format!(
            r#"<!doctype html>
<html lang="ru">
<body style="margin:0;padding:0;background:#0f1115;font-family:Segoe UI,Arial,sans-serif;color:#e6edf3;">
  <div style="max-width:520px;margin:0 auto;padding:40px 24px;">
    <div style="font-size:22px;font-weight:700;letter-spacing:2px;color:#7ee787;">LAVUNIO</div>
    <div style="margin-top:24px;background:#161b22;border:1px solid #30363d;border-radius:14px;padding:28px;">
      <h1 style="margin:0 0 12px;font-size:20px;">Подтвердите почту</h1>
      <p style="margin:0 0 20px;line-height:1.6;color:#9da7b3;">
        Вы зарегистрировались в Lavunio. Введите код ниже в лаунчере, чтобы подтвердить адрес
        <strong style="color:#e6edf3;">{email}</strong>.
      </p>
      <div style="margin:0 0 20px;padding:16px 20px;border:1px solid #30363d;border-radius:12px;background:#0d1117;text-align:center;font-size:34px;font-weight:800;letter-spacing:8px;color:#7ee787;">{code}</div>
      <p style="margin:20px 0 0;font-size:13px;line-height:1.6;color:#6e7681;">
        Код действует 15 минут. Если вы не регистрировались в Lavunio, просто проигнорируйте письмо.
      </p>
    </div>
    <p style="margin:20px 0 0;font-size:12px;color:#6e7681;">Lavunio — платформа игровых режимов Minecraft.</p>
  </div>
</body>
</html>"#
        );
        let text = format!(
            "Подтвердите почту в Lavunio\n\nКод подтверждения: {code}\n\nВведите его в лаунчере. Код действует 15 минут. Если вы не регистрировались, проигнорируйте письмо."
        );
        self.send(to, subject, &html, &text).await
    }

    pub async fn send_login_code(
        &self,
        to: &str,
        code: &str,
        ip: Option<&str>,
    ) -> Result<(), String> {
        let subject = "Новое место входа в Lavunio";
        let place = escape_html(ip.unwrap_or("неизвестное место"));
        let email = escape_html(to);
        let html = format!(
            r#"<!doctype html>
<html lang="ru">
<body style="margin:0;padding:0;background:#0f1115;font-family:Segoe UI,Arial,sans-serif;color:#e6edf3;">
  <div style="max-width:520px;margin:0 auto;padding:40px 24px;">
    <div style="font-size:22px;font-weight:700;letter-spacing:2px;color:#7ee787;">LAVUNIO</div>
    <div style="margin-top:24px;background:#161b22;border:1px solid #30363d;border-radius:14px;padding:28px;">
      <h1 style="margin:0 0 12px;font-size:20px;">Обнаружено новое место входа</h1>
      <p style="margin:0 0 20px;line-height:1.6;color:#9da7b3;">
        В аккаунт <strong style="color:#e6edf3;">{email}</strong> пытаются войти с нового места:
        <strong style="color:#e6edf3;">{place}</strong>. Если это вы, введите код ниже в лаунчере.
      </p>
      <div style="margin:0 0 20px;padding:16px 20px;border:1px solid #30363d;border-radius:12px;background:#0d1117;text-align:center;font-size:34px;font-weight:800;letter-spacing:8px;color:#7ee787;">{code}</div>
      <p style="margin:20px 0 0;font-size:13px;line-height:1.6;color:#6e7681;">
        Код действует 10 минут. Если вы не пытались войти, смените пароль и проигнорируйте письмо.
      </p>
    </div>
    <p style="margin:20px 0 0;font-size:12px;color:#6e7681;">Lavunio — платформа игровых режимов Minecraft.</p>
  </div>
</body>
</html>"#
        );
        let text = format!(
            "Новое место входа в Lavunio\n\nВ аккаунт {to} пытаются войти с нового места: {place}.\nКод подтверждения: {code}\n\nВведите его в лаунчере. Код действует 10 минут. Если вы не пытались войти, смените пароль."
        );
        self.send(to, subject, &html, &text).await
    }

    pub async fn send_password_reset(&self, to: &str, token: &str) -> Result<(), String> {
        let link = format!("{}/reset-password?token={token}", self.base_url);
        let subject = "Восстановление пароля Lavunio";
        let html = format!(
            r#"<!doctype html>
<html lang="ru">
<body style="margin:0;padding:0;background:#0f1115;font-family:Segoe UI,Arial,sans-serif;color:#e6edf3;">
  <div style="max-width:520px;margin:0 auto;padding:40px 24px;">
    <div style="font-size:22px;font-weight:700;letter-spacing:2px;color:#7ee787;">LAVUNIO</div>
    <div style="margin-top:24px;background:#161b22;border:1px solid #30363d;border-radius:14px;padding:28px;">
      <h1 style="margin:0 0 12px;font-size:20px;">Сброс пароля</h1>
      <p style="margin:0 0 20px;line-height:1.6;color:#9da7b3;">
        Мы получили запрос на восстановление пароля для аккаунта
        <strong style="color:#e6edf3;">{to}</strong>.
      </p>
      <a href="{link}" style="display:inline-block;background:#2ea043;color:#ffffff;text-decoration:none;font-weight:600;padding:12px 22px;border-radius:10px;">
        Задать новый пароль
      </a>
      <p style="margin:20px 0 0;font-size:13px;line-height:1.6;color:#6e7681;">
        Ссылка действует 1 час. Если вы не запрашивали сброс, просто проигнорируйте письмо — пароль останется прежним.
      </p>
      <p style="margin:16px 0 0;font-size:12px;line-height:1.6;color:#6e7681;word-break:break-all;">
        Если кнопка не работает, откройте ссылку: {link}
      </p>
    </div>
    <p style="margin:20px 0 0;font-size:12px;color:#6e7681;">Lavunio — платформа игровых режимов Minecraft.</p>
  </div>
</body>
</html>"#
        );
        let text = format!(
            "Восстановление пароля Lavunio\n\nОткройте ссылку, чтобы задать новый пароль для {to}:\n{link}\n\nСсылка действует 1 час. Если вы не запрашивали сброс, проигнорируйте письмо."
        );
        self.send(to, subject, &html, &text).await
    }

    async fn send(&self, to: &str, subject: &str, html: &str, text: &str) -> Result<(), String> {
        let api_key = self
            .api_key
            .as_deref()
            .ok_or_else(|| "RESEND_API_KEY is not configured".to_string())?;
        let mut payload = json!({
            "from": self.from,
            "to": [to],
            "subject": subject,
            "html": html,
            "text": text,
        });
        if let Some(reply_to) = &self.reply_to {
            payload["reply_to"] = json!([reply_to]);
        }
        let body = serde_json::to_vec(&payload).map_err(|error| error.to_string())?;
        let response = self
            .client
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| format!("resend request failed: {error}"))?;
        let status = response.status();
        let response_body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("resend error {status}: {response_body}"));
        }
        tracing::info!(to = %to, subject = %subject, "email sent");
        Ok(())
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
