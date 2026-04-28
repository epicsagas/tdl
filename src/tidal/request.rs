use anyhow::{anyhow, Result};
use reqwest::{Client, Response, StatusCode};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

const TIDAL_API_V1: &str = "https://api.tidal.com/v1/";
const TIDAL_API_V2: &str = "https://api.tidal.com/v2/";
const TIDAL_AUTH_URL: &str = "https://auth.tidal.com/v1/oauth2/";
const TIDAL_LOGIN_URL: &str = "https://login.tidal.com/";
const REQUESTS_TIMEOUT_SEC: u64 = 45;
const MAX_RETRIES: u32 = 5;
const BACKOFF_FACTOR: f64 = 1.0;

const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 12; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/91.0.4472.114 Safari/537.36";
const CLIENT_VERSION: &str = "2025.7.16";

pub const fn tidal_api_v1() -> &'static str {
    TIDAL_API_V1
}

pub const fn tidal_api_v2() -> &'static str {
    TIDAL_API_V2
}

pub const fn tidal_auth_url() -> &'static str {
    TIDAL_AUTH_URL
}

pub const fn tidal_login_url() -> &'static str {
    TIDAL_LOGIN_URL
}

pub struct TidalRequest {
    client: Client,
    access_token: Option<String>,
    token_type: Option<String>,
    session_id: Option<String>,
    country_code: Option<String>,
    client_version: String,
}

impl TidalRequest {
    /// Creates a new `TidalRequest` with a configured HTTP client.
    ///
    /// The client is initialised with a request timeout and a User-Agent header.
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(REQUESTS_TIMEOUT_SEC))
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {e}"))?;

        Ok(Self {
            client,
            access_token: None,
            token_type: None,
            session_id: None,
            country_code: None,
            client_version: CLIENT_VERSION.to_string(),
        })
    }

    /// Sets authentication credentials used for API requests.
    pub fn set_auth(&mut self, token_type: String, access_token: String) {
        self.token_type = Some(token_type);
        self.access_token = Some(access_token);
    }

    /// Sets session parameters that are appended as query parameters to V1 requests.
    pub fn set_session(&mut self, session_id: String, country_code: String) {
        self.session_id = Some(session_id);
        self.country_code = Some(country_code);
    }

    /// Updates just the country code used in V1 query parameters.
    pub fn set_country_code(&mut self, country_code: impl Into<String>) {
        self.country_code = Some(country_code.into());
    }

    /// Convenience method: send a V1 GET request and deserialize the JSON
    /// response into `T`.
    ///
    /// Returns a descriptive error if the API responds with a non-success
    /// status code or if the body cannot be decoded.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<T> {
        let resp = self.get_v1(path, params).await?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| {
            anyhow!("API {path}: failed to read response body ({status}): {e}")
        })?;

        if !status.is_success() {
            return Err(anyhow!(
                "API {path} returned {status}: {}",
                body.chars().take(500).collect::<String>()
            ));
        }

        serde_json::from_str::<T>(&body).map_err(|e| {
            anyhow!(
                "API {path}: JSON parse failed: {e} — body: {}",
                body.chars().take(300).collect::<String>()
            )
        })
    }

    /// Sends a GET request to the Tidal API V1 endpoint.
    ///
    /// The base URL is `TIDAL_API_V1`. The `path` argument is appended to that base.
    /// Authorisation and session query parameters are added automatically.
    /// A default `limit` of 10 000 is included unless the caller provides one.
    ///
    /// Retries on HTTP 429 (rate-limit) and 5xx responses using exponential back-off.
    pub async fn get_v1(
        &self,
        path: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<Response> {
        let url = format!("{TIDAL_API_V1}{path}");
        let mut query = params.unwrap_or_default();

        // Always include session parameters for V1 requests.
        if let Some(ref sid) = self.session_id {
            query.insert("sessionId".to_string(), sid.clone());
        }
        if let Some(ref cc) = self.country_code {
            query.insert("countryCode".to_string(), cc.clone());
        }
        // Use a generous default limit unless the caller overrides it.
        query.entry("limit".to_string()).or_insert("10000".to_string());

        self.send_with_retry(&url, &query).await
    }

    /// Sends a GET request to the Tidal API V2 endpoint, returning
    /// deserialized JSON. Checks for non-success status codes and
    /// shows the raw body on parse failure.
    pub async fn get_v2<T: DeserializeOwned>(
        &self,
        path: &str,
        params: Option<HashMap<String, String>>,
    ) -> Result<T> {
        let url = format!("{TIDAL_API_V2}{path}");
        let query = params.unwrap_or_default();

        let resp = self.send_with_retry(&url, &query).await?;
        let status = resp.status();
        let body = resp.text().await.map_err(|e| {
            anyhow!("API v2 {path}: failed to read response body ({status}): {e}")
        })?;

        if !status.is_success() {
            return Err(anyhow!(
                "API v2 {path} returned {status}: {}",
                body.chars().take(500).collect::<String>()
            ));
        }

        serde_json::from_str::<T>(&body).map_err(|e| {
            anyhow!(
                "API v2 {path}: JSON parse failed: {e} — body: {}",
                body.chars().take(300).collect::<String>()
            )
        })
    }

    /// Sends a form-encoded POST request to the Tidal authentication endpoint.
    ///
    /// The base URL is `TIDAL_AUTH_URL`. The `path` argument is appended to that base.
    pub async fn post_auth(
        &self,
        path: &str,
        form: HashMap<String, String>,
    ) -> Result<Response> {
        let url = format!("{TIDAL_AUTH_URL}{path}");

        self.client
            .post(&url)
            .form(&form)
            .send()
            .await
            .map_err(|e| anyhow!("Auth POST request failed for {path}: {e}"))
    }

    /// Sends a GET request to an arbitrary URL.
    ///
    /// This is intended for downloading raw binary content such as media segments.
    /// No authorisation headers or session parameters are added.
    pub async fn get_raw(&self, url: &str) -> Result<Response> {
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| anyhow!("Raw GET request failed for {url}: {e}"))
    }

    /// Sends a GET request to an arbitrary URL with session auth headers and
    /// countryCode query param attached. Used for authenticated CDN resources
    /// such as cover art.
    pub async fn get_v1_raw(&self, url: &str) -> Result<Response> {
        let mut query = HashMap::new();
        if let Some(ref cc) = self.country_code {
            query.insert("countryCode".to_string(), cc.clone());
        }
        self.send_with_retry(url, &query).await
    }

    // -- Internal helpers -----------------------------------------------------

    /// Executes a GET request with exponential-back-off retry logic.
    ///
    /// Retries are attempted when the server responds with HTTP 429 or a 5xx status,
    /// or when the request itself fails at the transport level.
    /// The back-off delay is `BACKOFF_FACTOR * 2^attempt` seconds.
    async fn send_with_retry(
        &self,
        url: &str,
        query: &HashMap<String, String>,
    ) -> Result<Response> {
        let mut attempt: u32 = 0;

        loop {
            let request = self
                .client
                .get(url)
                .query(query)
                .header("x-tidal-client-version", &self.client_version);

            let request = self.attach_auth_header(request);

            debug!(url = %url, attempt = attempt, "HTTP GET");
            match request.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status == StatusCode::TOO_MANY_REQUESTS
                        || status.as_u16() >= 500
                    {
                        attempt += 1;
                        if attempt > MAX_RETRIES {
                            return Err(anyhow!(
                                "Request to {url} failed after {MAX_RETRIES} retries (last status: {status})"
                            ));
                        }
                        let delay_secs = BACKOFF_FACTOR * 2f64.powi(attempt as i32);
                        warn!(url = %url, status = %status, attempt = attempt, delay_secs = delay_secs, "Rate-limited or server error, retrying");
                        sleep(Duration::from_secs_f64(delay_secs)).await;
                        continue;
                    }
                    debug!(url = %url, status = %status, "HTTP response");
                    return Ok(resp);
                }
                Err(e) => {
                    attempt += 1;
                    if attempt > MAX_RETRIES {
                        return Err(anyhow!(
                            "Request to {url} failed after {MAX_RETRIES} retries: {e}"
                        ));
                    }
                    let delay_secs = BACKOFF_FACTOR * 2f64.powi(attempt as i32);
                    warn!(url = %url, attempt = attempt, "Request error: {e}. Retrying in {delay_secs:.1}s");
                    sleep(Duration::from_secs_f64(delay_secs)).await;
                    continue;
                }
            }
        }
    }

    /// Attaches the Authorization header to a request builder when credentials are present.
    fn attach_auth_header(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let (Some(ttype), Some(token)) = (&self.token_type, &self.access_token) {
            builder.header("Authorization", format!("{ttype} {token}"))
        } else {
            builder
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_instance() {
        let req = TidalRequest::new();
        assert!(req.is_ok());
    }

    #[test]
    fn test_set_auth() {
        let mut req = TidalRequest::new().unwrap();
        req.set_auth("Bearer".to_string(), "test-token".to_string());
        assert_eq!(req.token_type, Some("Bearer".to_string()));
        assert_eq!(req.access_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_set_session() {
        let mut req = TidalRequest::new().unwrap();
        req.set_session("session-123".to_string(), "US".to_string());
        assert_eq!(req.session_id, Some("session-123".to_string()));
        assert_eq!(req.country_code, Some("US".to_string()));
    }

    #[test]
    fn test_constants() {
        assert!(tidal_api_v1().ends_with('/'));
        assert!(tidal_api_v2().ends_with('/'));
        assert!(tidal_auth_url().ends_with('/'));
        assert!(tidal_login_url().ends_with('/'));
    }
}
