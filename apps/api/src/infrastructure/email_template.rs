//! One table-based, inline-styled template for every transactional email.
//! Stored HTML and plain text travel together through the outbox and all transports.
use crate::error::{ApiError, Result};

pub(crate) struct Action<'a> {
    pub label: &'a str,
    pub url: &'a str,
}
pub(crate) struct Email {
    pub text: String,
    pub html: String,
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub(crate) fn render(
    brand: &str,
    title: &str,
    body: &str,
    action: Option<Action<'_>>,
) -> Result<Email> {
    let mut text = format!("{brand}\n\n{title}\n\n{body}");
    let paragraphs = body
        .split("\n\n")
        .map(|p| {
            format!(
                "<p style=\"margin:0 0 20px;line-height:1.7\">{}</p>",
                escape(p).replace('\n', "<br>")
            )
        })
        .collect::<String>();
    let cta = if let Some(action) = action {
        let url = reqwest::Url::parse(action.url).map_err(ApiError::internal)?;
        if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() {
            return Err(ApiError::bad("Email actions require an HTTP or HTTPS URL."));
        }
        text.push_str(&format!("\n\n{}:\n{}", action.label, action.url));
        let href = escape(action.url);
        format!(
            r##"<table role="presentation" cellspacing="0" cellpadding="0"><tr><td bgcolor="#147d64" style="border-radius:12px"><a href="{href}" style="display:inline-block;padding:14px 24px;border:1px solid #147d64;border-radius:12px;background:#147d64;color:#ffffff;font-weight:bold;text-decoration:none">{}</a></td></tr></table><p style="margin:24px 0 0;font-size:12px;line-height:1.7;color:#5e6d63">If the button doesn't work, copy this link into your browser:<br><a href="{href}" style="color:#147d64;word-break:break-all">{href}</a></p>"##,
            escape(action.label)
        )
    } else {
        String::new()
    };
    text.push_str(&format!(
        "\n\n{brand} · Space to build.\nThis is an account notification. Please do not reply."
    ));
    let brand = escape(brand);
    let title = escape(title);
    let preheader = escape(body.split("\n\n").next().unwrap_or_default());
    let html = format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{title}</title></head>
<body style="margin:0;padding:0;background:#f8f9f6;color:#25332d;font-family:Arial,Helvetica,sans-serif">
<div style="display:none;max-height:0;overflow:hidden;opacity:0">{preheader}</div>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" bgcolor="#f8f9f6"><tr><td align="center" style="padding:32px 16px">
<table role="presentation" width="600" cellspacing="0" cellpadding="0" style="width:100%;max-width:600px"><tr><td style="padding:0 8px 24px;font-size:24px;font-weight:bold;color:#147d64">✦ {brand}</td></tr>
<tr><td bgcolor="#ffffff" style="padding:32px;border:1px solid #e2e7df;border-radius:16px;font-size:16px"><h1 style="margin:0 0 24px;font-size:26px;line-height:1.3;color:#25332d">{title}</h1>{paragraphs}{cta}</td></tr>
<tr><td style="padding:24px 8px;color:#5e6d63;font-size:12px;line-height:1.7">{brand} · Space to build.<br>This is an account notification. Please do not reply.</td></tr></table>
</td></tr></table></body></html>"##
    );
    Ok(Email { text, html })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_all_untrusted_content_and_preserves_plain_text() {
        let email = render(
            "A&B",
            "<Title>",
            "Hi <script>alert(1)</script>",
            Some(Action {
                label: "Open <account>",
                url: "https://example.com/verify#token=a&other=b",
            }),
        )
        .unwrap();
        assert!(email.html.contains("A&amp;B"));
        assert!(!email.html.contains("<script>"));
        assert!(email.html.contains("Open &lt;account&gt;"));
        assert!(email.html.contains("token=a&amp;other=b"));
        assert!(
            email
                .text
                .contains("https://example.com/verify#token=a&other=b")
        );
        assert!(email.html.contains("role=\"presentation\""));
    }
    #[test]
    fn rejects_active_urls_and_supports_notifications_without_actions() {
        assert!(
            render(
                "App",
                "Title",
                "Body",
                Some(Action {
                    label: "Open",
                    url: "javascript:alert(1)"
                })
            )
            .is_err()
        );
        let email = render("App", "Title", "Body", None).unwrap();
        assert!(!email.html.contains("href="));
        assert!(email.text.contains("Body"));
    }
}
