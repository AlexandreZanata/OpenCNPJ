//! Outgoing e-mail: SMTP through `lettre` when `--smtp-url` is set, otherwise `.eml` files
//! in an outbox directory (private installs and tests). Sending never blocks a request.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use lettre::message::header::{ContentTransferEncoding, ContentType};
use lettre::message::{Body, Mailbox};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

enum Transport {
    Smtp(AsyncSmtpTransport<Tokio1Executor>),
    Outbox(PathBuf),
}

pub struct Mailer {
    transport: Transport,
    from: Mailbox,
    pub public_url: String,
}

impl Mailer {
    pub fn new(smtp_url: Option<&str>, from: &str, outbox: PathBuf, public_url: &str) -> Result<Mailer> {
        let from: Mailbox = from.parse().with_context(|| format!("bad --mail-from {from}"))?;
        let transport = match smtp_url.filter(|u| !u.is_empty()) {
            Some(u) => Transport::Smtp(AsyncSmtpTransport::<Tokio1Executor>::from_url(u).context("bad --smtp-url")?.build()),
            None => Transport::Outbox(outbox),
        };
        Ok(Mailer { transport, from, public_url: public_url.trim_end_matches('/').to_string() })
    }

    pub fn describe(&self) -> String {
        match &self.transport {
            Transport::Smtp(_) => "smtp".into(),
            Transport::Outbox(d) => format!("outbox {}", d.display()),
        }
    }

    pub async fn send(&self, to: &str, subject: &str, body: String) -> Result<()> {
        let msg = Message::builder()
            .from(self.from.clone())
            .to(to.parse().with_context(|| format!("bad recipient {to}"))?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(plain_body(body))?;
        match &self.transport {
            Transport::Smtp(t) => {
                t.send(msg).await.context("smtp")?;
            }
            Transport::Outbox(dir) => {
                tokio::fs::create_dir_all(dir).await?;
                let name = format!("{}-{}.eml", crate::accounts::now(), crate::accounts::random_b62(8));
                tokio::fs::write(dir.join(&name), msg.formatted()).await?;
                eprintln!("mail to {to} ({subject}) written to {}", dir.join(name).display());
            }
        }
        Ok(())
    }

    /// Sends in the background; failures are logged.
    pub fn spawn(self: &Arc<Self>, to: String, subject: &'static str, body: String) {
        let m = self.clone();
        tokio::spawn(async move {
            if let Err(e) = m.send(&to, subject, body).await {
                eprintln!("mail to {to} failed: {e:#}");
            }
        });
    }
}

/// ASCII text with lines under 998 bytes is sent as-is in 7 bit (RFC 5322): links and keys
/// stay on one line instead of being quoted-printable wrapped at 76 columns.
fn plain_body(text: String) -> Body {
    let ascii = text.bytes().all(|b| b.is_ascii() && b != 0) && text.lines().all(|l| l.len() < 998);
    if ascii {
        let crlf = text.replace("\r\n", "\n").replace('\n', "\r\n");
        Body::dangerous_pre_encoded(crlf.into_bytes(), ContentTransferEncoding::SevenBit)
    } else {
        Body::new(text)
    }
}

const FOOTER: &str =
    "\n--\nOpenCNPJ. Keep your key secret: anyone who has it can use your account.\nIf you did not request this, ignore this message.\n";

pub fn welcome(username: &str, key: &str, public_url: &str) -> String {
    format!(
        "Hello {username},\n\nYour OpenCNPJ account is ready. This is your login key:\n\n    {key}\n\n\
         Use it as `Authorization: Bearer <key>` on API requests, or log in with it at\n{public_url}/api/v1/auth/login.\n\n\
         There is no password. If you lose the key, request a new one with your e-mail address at\n{public_url}/api/v1/auth/recover.\n{FOOTER}"
    )
}

pub fn already_registered(username: &str, public_url: &str) -> String {
    format!(
        "Hello {username},\n\nSomeone tried to create an OpenCNPJ account with this e-mail address, which already has one.\n\
         If you lost your login key, request a new one at {public_url}/api/v1/auth/recover.\n{FOOTER}"
    )
}

pub fn recovery(username: &str, token: &str, public_url: &str) -> String {
    format!(
        "Hello {username},\n\nOpen this link within 30 minutes to replace your login key. Your current key stops working\n\
         when you confirm:\n\n    {public_url}/api/v1/auth/recover/confirm?token={token}\n\n\
         API clients can POST {{\"token\": \"{token}\"}} to {public_url}/api/v1/auth/recover/confirm instead.\n{FOOTER}"
    )
}

pub fn new_key(username: &str, key: &str, why: &str) -> String {
    format!(
        "Hello {username},\n\nYour OpenCNPJ login key was replaced ({why}). Your previous key and all sessions no longer work.\n\
         This is your new key:\n\n    {key}\n{FOOTER}"
    )
}

pub fn key_rotated_notice(username: &str, prefix: &str) -> String {
    format!(
        "Hello {username},\n\nYour OpenCNPJ login key was replaced from your account. The new key starts with ocnpj_{prefix}_.\n\
         If this was not you, request a new key through recovery right away.\n{FOOTER}"
    )
}
