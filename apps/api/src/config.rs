#[derive(Clone)]
pub struct Config {
    pub app_url: String,
    pub production: bool,
    pub mail_mode: String,
    pub resend_key: Option<String>,
    pub mail_from: String,
    pub mail_brand: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub stripe_key: Option<String>,
    pub stripe_price: Option<String>,
    pub stripe_webhook_secret: Option<String>,
}
impl Config {
    pub fn from_env() -> Self {
        let production = std::env::var("APP_ENV").as_deref() == Ok("production");
        let app_url = std::env::var("APP_URL")
            .unwrap_or_else(|_| "http://localhost:5173".into())
            .trim_end_matches('/')
            .to_owned();
        assert!(
            !production || app_url.starts_with("https://"),
            "Production APP_URL must use HTTPS"
        );
        let mail_mode = std::env::var("MAIL_MODE").unwrap_or_else(|_| "smtp".into());
        assert!(
            ["smtp", "console", "resend"].contains(&mail_mode.as_str()),
            "MAIL_MODE must be smtp, console, or resend"
        );
        assert!(
            !production || mail_mode == "resend",
            "Production requires MAIL_MODE=resend"
        );
        let optional = |key| std::env::var(key).ok().filter(|s| !s.is_empty());
        let resend_key = optional("RESEND_API_KEY");
        assert!(
            mail_mode != "resend" || resend_key.is_some(),
            "RESEND_API_KEY is required"
        );
        Self {
            app_url,
            production,
            mail_mode,
            resend_key,
            mail_from: std::env::var("MAIL_FROM")
                .unwrap_or_else(|_| "AppShell <onboarding@resend.dev>".into()),
            mail_brand: std::env::var("MAIL_BRAND").unwrap_or_else(|_| "AppShell".into()),
            smtp_host: std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".into()),
            smtp_port: std::env::var("SMTP_PORT").map_or(1025, |value| {
                value
                    .parse::<u16>()
                    .expect("SMTP_PORT must be a valid port")
            }),
            stripe_key: optional("STRIPE_SECRET_KEY"),
            stripe_price: optional("STRIPE_PRICE_ID"),
            stripe_webhook_secret: optional("STRIPE_WEBHOOK_SECRET"),
        }
    }
    pub fn billing_enabled(&self) -> bool {
        self.stripe_key.is_some()
            && self.stripe_price.is_some()
            && self.stripe_webhook_secret.is_some()
    }
}
