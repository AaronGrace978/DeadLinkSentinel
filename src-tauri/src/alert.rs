use lettre::message::{Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

use crate::db;
use crate::db::sites::Site;
use crate::error::{AppError, AppResult};
use crate::secrets;
use crate::state::AppState;

pub async fn maybe_send_regression(
    state: &AppState,
    site: &Site,
    scan_id: i64,
) -> AppResult<()> {
    let conn = db::conn(&state.db)?;
    let settings = db::settings::load(&conn)?;
    if settings.smtp_host.is_empty() || settings.smtp_to.is_empty() || settings.smtp_from.is_empty()
    {
        return Ok(());
    }

    let current = db::scans::broken_targets(&conn, scan_id)?;
    let previous = db::scans::previous_completed(&conn, site.id, scan_id)?;
    let prev_set: std::collections::HashSet<String> = if let Some(prev) = previous {
        db::scans::broken_targets(&conn, prev.id)?
            .into_iter()
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let new_breaks: Vec<String> = current
        .iter()
        .filter(|u| !prev_set.contains(*u))
        .cloned()
        .collect();
    if new_breaks.is_empty() {
        return Ok(());
    }

    let password = secrets::get(&conn, secrets::SMTP_PASSWORD).unwrap_or_default();
    drop(conn);

    let subject = format!(
        "[DeadLinkSentinel] {} — {} new broken link{}",
        site.name,
        new_breaks.len(),
        if new_breaks.len() == 1 { "" } else { "s" }
    );
    let mut body = format!(
        "Site: {}\nSeed: {}\nNew or regressed broken links ({}):\n\n",
        site.name,
        site.seed_url,
        new_breaks.len()
    );
    for url in new_breaks.iter().take(50) {
        body.push_str("- ");
        body.push_str(url);
        body.push('\n');
    }
    if new_breaks.len() > 50 {
        body.push_str(&format!("\n…and {} more.\n", new_breaks.len() - 50));
    }
    body.push_str("\nScans and the public status page are only available while DeadLinkSentinel is running.\n");

    send_email(&settings, &password, &subject, &body).await
}

pub async fn send_test(state: &AppState) -> AppResult<String> {
    let conn = db::conn(&state.db)?;
    let settings = db::settings::load(&conn)?;
    let password = secrets::get(&conn, secrets::SMTP_PASSWORD).unwrap_or_default();
    drop(conn);
    if settings.smtp_host.is_empty() || settings.smtp_to.is_empty() || settings.smtp_from.is_empty()
    {
        return Err(AppError::msg(
            "SMTP host, from, and to addresses are required",
        ));
    }
    send_email(
        &settings,
        &password,
        "[DeadLinkSentinel] Test email",
        "SMTP is configured. You will receive regression alerts when new broken links appear.",
    )
    .await?;
    Ok(format!("Test email sent to {}", settings.smtp_to))
}

async fn send_email(
    settings: &db::settings::AppSettings,
    password: &str,
    subject: &str,
    body: &str,
) -> AppResult<()> {
    let from: Mailbox = settings.smtp_from.parse()?;
    let to: Mailbox = settings.smtp_to.parse()?;
    let msg = Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .body(body.to_string())?;

    let mut builder = match settings.smtp_tls.as_str() {
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.smtp_host)
            .port(settings.smtp_port)
            .tls(Tls::None),
        "tls" | "wrapper" => {
            let tls_params = TlsParameters::new(settings.smtp_host.clone())
                .map_err(|e| AppError::msg(e.to_string()))?;
            AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.smtp_host)
                .map_err(|e| AppError::msg(e.to_string()))?
                .port(settings.smtp_port)
                .tls(Tls::Wrapper(tls_params))
        }
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.smtp_host)
            .map_err(|e| AppError::msg(e.to_string()))?
            .port(settings.smtp_port),
    };

    if !settings.smtp_user.is_empty() {
        builder = builder.credentials(Credentials::new(
            settings.smtp_user.clone(),
            password.to_string(),
        ));
    }

    let mailer = builder.build();
    mailer.send(msg).await?;
    Ok(())
}
