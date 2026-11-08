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

    pub async fn send_verification(&self, to: &str, token: &str) -> Result<(), String> {
        let link = format!("{}/v1/auth/verify-email?token={token}", self.base_url);
        let subject = "Подтвердите почту в Lavunio";
        let html = format!(
            r#"<!doctype html>
<html lang="ru">
<body style="margin:0;padding:0;background:#0f1115;font-family:Segoe UI,Arial,sans-serif;color:#e6edf3;">
  <div style="max-width:520px;margin:0 auto;padding:40px 24px;">
    <div style="font-size:22px;font-weight:700;letter-spacing:2px;color:#7ee787;">LAVUNIO</div>
    <div style="margin-top:24px;background:#161b22;border:1px solid #30363d;border-radius:14px;padding:28px;">
      <h1 style="margin:0 0 12px;font-size:20px;">Подтвердите почту</h1>
      <p style="margin:0 0 20px;line-height:1.6;color:#9da7b3;">
        Вы зарегистрировались в Lavunio. Нажмите кнопку ниже, чтобы подтвердить адрес
        <strong style="color:#e6edf3;">{to}</strong>.
      </p>
      <a href="{link}" style="display:inline-block;background:#2ea043;color:#ffffff;text-decoration:none;font-weight:600;padding:12px 22px;border-radius:10px;">
        Подтвердить почту
      </a>
      <p style="margin:20px 0 0;font-size:13px;line-height:1.6;color:#6e7681;">
        Ссылка действует 24 часа. Если вы не регистрировались в Lavunio, просто проигнорируйте письмо.
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
            "Подтвердите почту в Lavunio\n\nОткройте ссылку, чтобы подтвердить {to}:\n{link}\n\nСсылка действует 24 часа. Если вы не регистрировались, проигнорируйте письмо."
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
