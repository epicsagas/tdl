use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use rand::RngExt;


use crate::config::settings::Settings;
use crate::config::token::Token;
use crate::tidal::media::{DeviceAuthResponse, SessionResponse, TokenResponse};
use crate::tidal::request::TidalRequest;

// ---------------------------------------------------------------------------
// Double-base64-encoded client credentials.
//
// Decoding: concat the two halves (each half is base64 that decodes to a
// fragment of another base64 string), base64-decode the concatenation to get
// a second base64 string, then base64-decode that again for the plaintext.
// ---------------------------------------------------------------------------

const OAUTH_CLIENT_ID_PARTS: (&str, &str) =
    ("WmxneVNuaGtiVzUw", "V2xkTE1HbDRWQT09");
const OAUTH_CLIENT_SECRET_PARTS: (&str, &str) = (
    "TVU1dU9VRm1SRUZxZUhKblNrWktZa3RPVjB4bFFY",
    "bExSMVpIYlVsT2RWaFFVRXhJVmxoQmRuaEJaejA9",
);
const PKCE_CLIENT_ID_PARTS: (&str, &str) = ("TmtKRVUxSmtjRXM=", "NWFIRkZRbFJuVlE9PQ==");
const PKCE_CLIENT_SECRET_PARTS: (&str, &str) = (
    "ZUdWMVVHMVpOMjVpY0ZvNVNVbGlURUZqVVQ=",
    "a3pjMmhyWVRGV1RtaGxWVUZ4VGpaSlkzTjZhbFJIT0QwPQ==",
);

/// Decode a double-base64-encoded credential (two parts concatenated, then
/// decoded twice).
fn decode_credential(part1: &str, part2: &str) -> Result<String> {
    let first_pass = format!(
        "{}{}",
        String::from_utf8(BASE64.decode(part1)?)?,
        String::from_utf8(BASE64.decode(part2)?)?
    );
    let raw = BASE64.decode(&first_pass)?;
    String::from_utf8(raw).context("Decoded credential is not valid UTF-8")
}

const SCOPE: &str = "r_usr w_usr w_sub";

// ---------------------------------------------------------------------------
// TidalSession
// ---------------------------------------------------------------------------

pub struct TidalSession {
    pub request: TidalRequest,
    pub token: Token,
    pub settings: Settings,
    pub client_id: String,
    pub client_secret: String,
    pub pkce_client_id: String,
    pub pkce_client_secret: String,
}

impl TidalSession {
    /// Create a new session, loading the token from disk and decoding the
    /// embedded client credentials.
    pub fn new(settings: Settings) -> Result<Self> {
        let token = Token::load().unwrap_or_default();
        let request = TidalRequest::new()?;

        let client_id = decode_credential(OAUTH_CLIENT_ID_PARTS.0, OAUTH_CLIENT_ID_PARTS.1)?;
        let client_secret =
            decode_credential(OAUTH_CLIENT_SECRET_PARTS.0, OAUTH_CLIENT_SECRET_PARTS.1)?;
        let pkce_client_id = decode_credential(PKCE_CLIENT_ID_PARTS.0, PKCE_CLIENT_ID_PARTS.1)?;
        let pkce_client_secret =
            decode_credential(PKCE_CLIENT_SECRET_PARTS.0, PKCE_CLIENT_SECRET_PARTS.1)?;

        Ok(Self {
            request,
            token,
            settings,
            client_id,
            client_secret,
            pkce_client_id,
            pkce_client_secret,
        })
    }

    // -----------------------------------------------------------------------
    // Login flow
    // -----------------------------------------------------------------------

    /// Attempt to establish an authenticated session.
    ///
    /// 1. If the stored token is still valid, validate it against the API.
    /// 2. Otherwise try to refresh the token.
    /// 3. Fall back to the full OAuth device-authorization flow.
    pub async fn login(&mut self) -> Result<()> {
        // 1. Token looks valid locally -- verify with the API.
        if self.token.is_valid() {
            self.apply_auth_to_request();
            if let Ok(session) = self.validate_session().await {
                self.set_session_info(&session);
                println!(
                    "Already logged in as user {}.",
                    session.user_id.unwrap_or(0)
                );
                return Ok(());
            }
        }

        // 2. Try a token refresh.
        if self.token.refresh_token.is_some() {
            println!("Access token expired, refreshing...");
            if self.refresh_token().await.is_ok() {
                let session = self.validate_session().await?;
                self.set_session_info(&session);
                println!("Token refreshed successfully.");
                return Ok(());
            }
        }

        // 3. Full device-authorization login.
        self.device_auth_login(|url, code| {
            println!("Visit {} and enter the code: {}", url, code);
        })
        .await
    }

    /// Like `login` but calls `url_handler(url, code)` instead of printing.
    /// Used by GUI to open the browser and emit the URL to the frontend.
    pub async fn login_with_url_handler<F>(&mut self, url_handler: F) -> Result<()>
    where
        F: FnOnce(&str, &str),
    {
        if self.token.is_valid() {
            self.apply_auth_to_request();
            if let Ok(session) = self.validate_session().await {
                self.set_session_info(&session);
                return Ok(());
            }
        }
        if self.token.refresh_token.is_some() {
            if self.refresh_token().await.is_ok() {
                let session = self.validate_session().await?;
                self.set_session_info(&session);
                return Ok(());
            }
        }
        self.device_auth_login(url_handler).await
    }

    /// Push current token credentials into the TidalRequest helper.
    fn apply_auth_to_request(&mut self) {
        if let (Some(ttype), Some(access)) =
            (&self.token.token_type, &self.token.access_token)
        {
            self.request.set_auth(ttype.clone(), access.clone());
        }
    }

    /// Set session-level fields on both the token and the request helper.
    fn set_session_info(&mut self, session: &SessionResponse) {
        if let Some(ref session_id) = session.session_id {
            self.token.session_id = Some(session_id.clone());
        }
        if let Some(ref country_code) = session.country_code {
            self.token.country_code = Some(country_code.clone());
        }
        if let (Some(session_id), Some(country_code)) =
            (&session.session_id, &session.country_code)
        {
            self.request
                .set_session(session_id.clone(), country_code.clone());
        }
        if let Some(user_id) = session.user_id {
            self.token.user_id = Some(user_id);
        }
    }

    // -----------------------------------------------------------------------
    // Refresh
    // -----------------------------------------------------------------------

    /// Refresh the access token using the stored refresh token.
    pub async fn refresh_token(&mut self) -> Result<()> {
        let refresh_token = self
            .token
            .refresh_token
            .as_ref()
            .ok_or_else(|| anyhow!("No refresh token available"))?
            .clone();

        let (client_id, client_secret) = if self.token.is_pkce {
            (self.pkce_client_id.clone(), self.pkce_client_secret.clone())
        } else {
            (self.client_id.clone(), self.client_secret.clone())
        };

        let mut form = HashMap::new();
        form.insert("grant_type".to_string(), "refresh_token".to_string());
        form.insert("refresh_token".to_string(), refresh_token);
        form.insert("client_id".to_string(), client_id);
        form.insert("client_secret".to_string(), client_secret);

        let resp = self.request.post_auth("token", form).await?;
        let token_resp: TokenResponse = resp.json().await?;

        if let Some(ref err) = token_resp.error {
            bail!(
                "Token refresh failed: {} ({})",
                token_resp.error_description.as_deref().unwrap_or("unknown"),
                err
            );
        }

        self.apply_token_response(&token_resp);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Session validation
    // -----------------------------------------------------------------------

    /// Validate the current access token by calling the sessions endpoint.
    pub async fn validate_session(&mut self) -> Result<SessionResponse> {
        let resp = self.request.get_v1("sessions", None).await?;
        let session: SessionResponse = resp.json().await?;
        Ok(session)
    }

    // -----------------------------------------------------------------------
    // OAuth device-authorization flow
    // -----------------------------------------------------------------------

    /// Perform the full device-authorization login flow.
    ///
    /// `url_handler` is called with `(login_url, user_code)` so the caller can
    /// open a browser, emit a GUI event, or just print to the terminal.
    async fn device_auth_login<F>(&mut self, url_handler: F) -> Result<()>
    where
        F: FnOnce(&str, &str),
    {
        // Step 1: Request device authorization.
        let mut form = HashMap::new();
        form.insert("client_id".to_string(), self.client_id.clone());
        form.insert("scope".to_string(), SCOPE.to_string());

        let resp = self
            .request
            .post_auth("device_authorization", form)
            .await?;
        let auth: DeviceAuthResponse = resp.json().await?;

        // The API may or may not return verificationUriComplete.
        // If present, use it directly; otherwise build it from verification_uri + user_code.
        let raw_url = auth
            .verification_uri_complete
            .clone()
            .unwrap_or_else(|| format!("{}/{}", auth.verification_uri, auth.user_code));
        let login_url = if raw_url.starts_with("http://") || raw_url.starts_with("https://") {
            raw_url
        } else {
            format!("https://{}", raw_url)
        };

        url_handler(&login_url, &auth.user_code);

        // Step 2: Poll for the token.
        let interval = Duration::from_secs(auth.interval);
        let deadline = SystemTime::now() + Duration::from_secs(auth.expires_in);

        let mut poll_form = HashMap::new();
        poll_form.insert("client_id".to_string(), self.client_id.clone());
        poll_form.insert("client_secret".to_string(), self.client_secret.clone());
        poll_form.insert("device_code".to_string(), auth.device_code.clone());
        poll_form.insert(
            "grant_type".to_string(),
            "urn:ietf:params:oauth:grant-type:device_code".to_string(),
        );
        poll_form.insert("scope".to_string(), SCOPE.to_string());

        loop {
            if SystemTime::now() > deadline {
                bail!("Device authorization timed out");
            }

            tokio::time::sleep(interval).await;

            let resp = self.request.post_auth("token", poll_form.clone()).await?;
            let token_resp: TokenResponse = resp.json().await?;

            match (&token_resp.error, &token_resp.access_token) {
                // "authorization_pending" -- user hasn't authorized yet.
                (Some(e), _) if e == "authorization_pending" => continue,
                // "slow_down" -- back off.
                (Some(e), _) if e == "slow_down" => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                // Other error.
                (Some(e), _) => {
                    bail!(
                        "Device authorization failed: {} ({})",
                        token_resp.error_description.as_deref().unwrap_or("unknown"),
                        e
                    );
                }
                // Success.
                (None, Some(_)) => {
                    self.apply_token_response(&token_resp);
                    let session = self.validate_session().await?;
                    self.set_session_info(&session);
                    println!("Login successful.");
                    return Ok(());
                }
                (None, None) => {
                    bail!("Device authorization returned no token and no error");
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // PKCE flow (for HiRes Lossless)
    // -----------------------------------------------------------------------

    /// Perform PKCE-based login for HiRes Lossless quality (CLI — reads redirect URL from stdin).
    pub async fn login_pkce(&mut self) -> Result<()> {
        let (auth_url, code_verifier, client_unique_key) = self.pkce_build_auth_url();

        println!("Open the following URL in a browser to log in:");
        println!("{}", auth_url);
        println!();
        print!("After login, paste the full redirect URL here: ");
        io::stdout().flush()?;

        let mut redirect_input = String::new();
        io::stdin().read_line(&mut redirect_input)?;
        let redirect_input = redirect_input.trim().to_string();

        self.pkce_exchange_code(&redirect_input, &code_verifier, &client_unique_key).await
    }

    /// Build PKCE auth URL and return (auth_url, code_verifier, client_unique_key).
    /// Called by both CLI and GUI flows.
    pub fn pkce_build_auth_url(&self) -> (String, String, String) {
        let key_bytes: [u8; 16] = rand::rng().random();
        let client_unique_key = hex::encode(key_bytes);

        let verifier_bytes: [u8; 32] = rand::rng().random();
        let code_verifier = BASE64URL.encode(verifier_bytes);

        let hash = Sha256::digest(code_verifier.as_bytes());
        let code_challenge = BASE64URL.encode(hash);

        let redirect_uri = "https://tidal.com/android/login/auth";
        let auth_url = format!(
            "https://login.tidal.com/authorize?response_type=code&redirect_uri={redirect_uri}&client_id={}&lang=EN&appMode=android&client_unique_key={}&code_challenge={}&code_challenge_method=S256",
            self.pkce_client_id,
            client_unique_key,
            code_challenge,
        );

        (auth_url, code_verifier, client_unique_key)
    }

    /// Exchange the PKCE authorization code (extracted from redirect_url) for tokens.
    /// Called by both CLI and GUI flows after the user completes browser login.
    pub async fn pkce_exchange_code(
        &mut self,
        redirect_url: &str,
        code_verifier: &str,
        client_unique_key: &str,
    ) -> Result<()> {
        let redirect_uri = "https://tidal.com/android/login/auth";
        let code = Self::extract_code_from_redirect(redirect_url)?;

        let mut form = HashMap::new();
        form.insert("code".to_string(), code);
        form.insert("client_id".to_string(), self.pkce_client_id.clone());
        form.insert("grant_type".to_string(), "authorization_code".to_string());
        form.insert("redirect_uri".to_string(), redirect_uri.to_string());
        form.insert("scope".to_string(), SCOPE.replace(' ', "+"));
        form.insert("code_verifier".to_string(), code_verifier.to_string());
        form.insert("client_unique_key".to_string(), client_unique_key.to_string());

        let resp = self.request.post_auth("token", form).await?;
        let token_resp: TokenResponse = resp.json().await?;

        if let Some(ref err) = token_resp.error {
            bail!(
                "PKCE token exchange failed: {} ({})",
                token_resp.error_description.as_deref().unwrap_or("unknown"),
                err
            );
        }

        self.token.is_pkce = true;
        self.apply_token_response(&token_resp);
        let session = self.validate_session().await?;
        self.set_session_info(&session);
        Ok(())
    }

    /// Extract the `code` query parameter from the redirect URL.
    fn extract_code_from_redirect(url: &str) -> Result<String> {
        // Try to parse as a URL first.
        if let Ok(parsed) = url::Url::parse(url) {
            for (key, value) in parsed.query_pairs() {
                if key == "code" {
                    return Ok(value.to_string());
                }
            }
        }

        // Fallback: simple string scan for code= in the URL.
        if let Some(start) = url.find("code=") {
            let rest = &url[start + 5..];
            let end = rest.find('&').unwrap_or(rest.len());
            let code = &rest[..end];
            if !code.is_empty() {
                return Ok(code.to_string());
            }
        }

        bail!("Could not extract authorization code from redirect URL")
    }

    // -----------------------------------------------------------------------
    // Logout
    // -----------------------------------------------------------------------

    /// Log out by deleting the stored token file.
    pub fn logout(&self) -> Result<()> {
        Token::delete()
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Apply a successful token response to the session state and persist it.
    fn apply_token_response(&mut self, resp: &TokenResponse) {
        if let Some(ref access_token) = resp.access_token {
            self.token.access_token = Some(access_token.clone());
        }
        if let Some(ref refresh_token) = resp.refresh_token {
            self.token.refresh_token = Some(refresh_token.clone());
        }
        if let Some(ref token_type) = resp.token_type {
            self.token.token_type = Some(token_type.clone());
        }
        if let Some(expires_in) = resp.expires_in {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64();
            self.token.expiry_time = now + expires_in as f64;
        }

        // Push the new credentials into the request helper.
        self.apply_auth_to_request();

        // Best-effort save -- should not fail the login flow.
        if let Err(e) = self.token.save() {
            eprintln!("Warning: failed to save token: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_oauth_client_id() {
        let val = decode_credential(OAUTH_CLIENT_ID_PARTS.0, OAUTH_CLIENT_ID_PARTS.1).unwrap();
        assert_eq!(val, "fX2JxdmntZWK0ixT");
    }

    #[test]
    fn decode_oauth_client_secret() {
        let val =
            decode_credential(OAUTH_CLIENT_SECRET_PARTS.0, OAUTH_CLIENT_SECRET_PARTS.1).unwrap();
        assert_eq!(val, "1Nn9AfDAjxrgJFJbKNWLeAyKGVGmINuXPPLHVXAvxAg=");
    }

    #[test]
    fn decode_pkce_client_id() {
        let val = decode_credential(PKCE_CLIENT_ID_PARTS.0, PKCE_CLIENT_ID_PARTS.1).unwrap();
        assert_eq!(val, "6BDSRdpK9hqEBTgU");
    }

    #[test]
    fn decode_pkce_client_secret() {
        let val =
            decode_credential(PKCE_CLIENT_SECRET_PARTS.0, PKCE_CLIENT_SECRET_PARTS.1).unwrap();
        assert_eq!(val, "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8=");
    }

    #[test]
    fn extract_code_from_redirect_url() {
        let url = "https://tidal.com/android/login/auth?code=abc123&state=xyz";
        let code = TidalSession::extract_code_from_redirect(url).unwrap();
        assert_eq!(code, "abc123");
    }

    #[test]
    fn extract_code_from_redirect_url_only_code_param() {
        let url = "https://tidal.com/android/login/auth?code=singleparam";
        let code = TidalSession::extract_code_from_redirect(url).unwrap();
        assert_eq!(code, "singleparam");
    }

    #[test]
    fn extract_code_from_redirect_fallback() {
        let input = "code=fallback123&other=yes";
        let code = TidalSession::extract_code_from_redirect(input).unwrap();
        assert_eq!(code, "fallback123");
    }

    #[test]
    fn extract_code_from_redirect_failure() {
        let input = "https://example.com/no_code_here";
        assert!(TidalSession::extract_code_from_redirect(input).is_err());
    }

    #[test]
    fn new_session_creates_with_defaults() {
        let settings = Settings::default();
        let session = TidalSession::new(settings).unwrap();
        assert_eq!(session.client_id, "fX2JxdmntZWK0ixT");
        assert_eq!(session.pkce_client_id, "6BDSRdpK9hqEBTgU");
    }
}
