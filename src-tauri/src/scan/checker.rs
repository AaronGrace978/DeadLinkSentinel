use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct LinkCheck {
    pub status_code: Option<u16>,
    pub final_url: Option<String>,
    pub error: Option<String>,
    pub is_broken: bool,
}

pub fn is_broken_code(code: u16) -> bool {
    code >= 400
}

pub async fn check_url(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
) -> LinkCheck {
    match send(client, reqwest::Method::HEAD, url, timeout).await {
        Ok(check) => {
            if matches!(check.status_code, Some(405 | 501)) {
                send(client, reqwest::Method::GET, url, timeout)
                    .await
                    .unwrap_or_else(err_check)
            } else {
                check
            }
        }
        Err(_) => send(client, reqwest::Method::GET, url, timeout)
            .await
            .unwrap_or_else(err_check),
    }
}

async fn send(
    client: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    timeout: Duration,
) -> Result<LinkCheck, String> {
    let resp = client
        .request(method, url)
        .timeout(timeout)
        .header(
            "Accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        )
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let code = resp.status().as_u16();
    let final_url = resp.url().to_string();
    Ok(LinkCheck {
        status_code: Some(code),
        final_url: Some(final_url),
        error: None,
        is_broken: is_broken_code(code),
    })
}

fn err_check(error: String) -> LinkCheck {
    LinkCheck {
        status_code: None,
        final_url: None,
        error: Some(error),
        is_broken: true,
    }
}

pub async fn fetch_html(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
) -> Result<(String, String, u16), String> {
    let resp = client
        .get(url)
        .timeout(timeout)
        .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let code = resp.status().as_u16();
    let final_url = resp.url().to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    // Cap HTML we parse.
    let slice = if bytes.len() > 2_000_000 {
        &bytes[..2_000_000]
    } else {
        &bytes
    };
    let html = String::from_utf8_lossy(slice).to_string();
    Ok((final_url, html, code))
}
