use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Duration as ChronoDuration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    env,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use url::Url;

const SANDBOX_BASE_URL: &str = "https://sandbox.safaricom.co.ke";
const TOKEN_REFRESH_BUFFER_SECONDS: u64 = 60;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    CustomerPayBillOnline,
    CustomerBuyGoodsOnline,
}

impl TransactionType {
    fn as_str(self) -> &'static str {
        match self {
            Self::CustomerPayBillOnline => "CustomerPayBillOnline",
            Self::CustomerBuyGoodsOnline => "CustomerBuyGoodsOnline",
        }
    }
}

impl FromStr for TransactionType {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "CustomerPayBillOnline" => Ok(Self::CustomerPayBillOnline),
            "CustomerBuyGoodsOnline" => Ok(Self::CustomerBuyGoodsOnline),
            _ => bail!(
                "DARAJA_TRANSACTION_TYPE must be CustomerPayBillOnline or CustomerBuyGoodsOnline"
            ),
        }
    }
}

#[derive(Clone)]
pub struct DarajaConfig {
    pub base_url: String,
    pub consumer_key: String,
    pub consumer_secret: String,
    pub shortcode: String,
    pub passkey: String,
    pub callback_url: String,
    pub transaction_type: TransactionType,
}

#[derive(Clone)]
pub struct DarajaClient {
    config: Arc<DarajaConfig>,
    http: Client,
    token: Arc<RwLock<Option<CachedToken>>>,
}

#[derive(Clone)]
struct CachedToken {
    value: String,
    refresh_after: Instant,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ExpiresIn {
    Seconds(u64),
    String(String),
}

#[derive(Deserialize)]
struct OAuthResponse {
    access_token: String,
    expires_in: ExpiresIn,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct StkPushRequest {
    business_short_code: String,
    password: String,
    timestamp: String,
    transaction_type: String,
    amount: String,
    party_a: String,
    party_b: String,
    phone_number: String,
    #[serde(rename = "CallBackURL")]
    call_back_url: String,
    account_reference: String,
    transaction_desc: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct StkPushResponse {
    #[serde(rename = "MerchantRequestID")]
    merchant_request_id: String,
    #[serde(rename = "CheckoutRequestID")]
    checkout_request_id: String,
    response_code: String,
    response_description: String,
}

pub struct StkPushResult {
    pub merchant_request_id: String,
    pub checkout_request_id: String,
}

impl DarajaClient {
    pub fn from_env() -> Result<Option<Self>> {
        let consumer_key = optional_env("DARAJA_CONSUMER_KEY")?;
        let consumer_secret = optional_env("DARAJA_CONSUMER_SECRET")?;
        let shortcode = optional_env("DARAJA_SHORTCODE")?;
        let passkey = optional_env("DARAJA_PASSKEY")?;
        let callback_url = optional_env("DARAJA_CALLBACK_URL")?;
        let transaction_type = optional_env("DARAJA_TRANSACTION_TYPE")?;

        if [
            &consumer_key,
            &consumer_secret,
            &shortcode,
            &passkey,
            &callback_url,
            &transaction_type,
        ]
        .iter()
        .all(|value| value.is_none())
        {
            return Ok(None);
        }

        let consumer_key = require_env_value("DARAJA_CONSUMER_KEY", consumer_key)?;
        let consumer_secret = require_env_value("DARAJA_CONSUMER_SECRET", consumer_secret)?;
        let shortcode = require_env_value("DARAJA_SHORTCODE", shortcode)?;
        let passkey = require_env_value("DARAJA_PASSKEY", passkey)?;
        let callback_url = require_env_value("DARAJA_CALLBACK_URL", callback_url)?;
        let transaction_type = require_env_value("DARAJA_TRANSACTION_TYPE", transaction_type)?
            .parse::<TransactionType>()?;
        let base_url =
            optional_env("DARAJA_BASE_URL")?.unwrap_or_else(|| SANDBOX_BASE_URL.to_owned());

        validate_https_url("DARAJA_BASE_URL", &base_url)?;
        validate_https_url("DARAJA_CALLBACK_URL", &callback_url)?;
        if !(5..=6).contains(&shortcode.len())
            || !shortcode.bytes().all(|byte| byte.is_ascii_digit())
        {
            bail!("DARAJA_SHORTCODE must contain 5 or 6 digits");
        }

        Self::new(DarajaConfig {
            base_url,
            consumer_key,
            consumer_secret,
            shortcode,
            passkey,
            callback_url,
            transaction_type,
        })
        .map(Some)
    }

    pub fn new(config: DarajaConfig) -> Result<Self> {
        Ok(Self {
            config: Arc::new(config),
            http: Client::builder()
                .build()
                .context("building Daraja HTTP client")?,
            token: Arc::new(RwLock::new(None)),
        })
    }

    pub async fn initiate_stk_push(
        &self,
        phone_number: &str,
        amount: i64,
    ) -> Result<StkPushResult> {
        let access_token = self.access_token().await?;
        let request = self.build_stk_request(phone_number, amount);
        let url = format!(
            "{}/mpesa/stkpush/v1/processrequest",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self
            .http
            .post(url)
            .bearer_auth(access_token)
            .json(&request)
            .send()
            .await
            .context("sending Daraja STK Push request")?;

        let status = response.status();
        if !status.is_success() {
            bail!("Daraja STK Push endpoint returned HTTP {status}");
        }

        let response = response
            .json::<StkPushResponse>()
            .await
            .context("parsing Daraja STK Push response")?;
        if response.response_code != "0" {
            bail!(
                "Daraja rejected STK Push request: {} ({})",
                response.response_description,
                response.response_code
            );
        }

        if response.merchant_request_id.is_empty() || response.checkout_request_id.is_empty() {
            bail!("Daraja STK Push response omitted request identifiers");
        }

        Ok(StkPushResult {
            merchant_request_id: response.merchant_request_id,
            checkout_request_id: response.checkout_request_id,
        })
    }

    async fn access_token(&self) -> Result<String> {
        if let Some(token) = self.usable_cached_token().await {
            return Ok(token);
        }

        let mut cached = self.token.write().await;
        if let Some(token) = cached
            .as_ref()
            .filter(|token| Instant::now() < token.refresh_after)
        {
            return Ok(token.value.clone());
        }

        let url = format!(
            "{}/oauth/v1/generate?grant_type=client_credentials",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self
            .http
            .get(url)
            .basic_auth(
                &self.config.consumer_key,
                Some(&self.config.consumer_secret),
            )
            .send()
            .await
            .context("requesting Daraja OAuth token")?;
        let status = response.status();
        if !status.is_success() {
            bail!("Daraja OAuth endpoint returned HTTP {status}");
        }

        let response = response
            .json::<OAuthResponse>()
            .await
            .context("parsing Daraja OAuth response")?;
        if response.access_token.is_empty() {
            bail!("Daraja OAuth response omitted access_token");
        }
        let expires_in = response.expires_in.seconds()?;
        if expires_in == 0 {
            bail!("Daraja OAuth response supplied a zero-second token lifetime");
        }

        let refresh_after = Instant::now()
            + Duration::from_secs(expires_in.saturating_sub(TOKEN_REFRESH_BUFFER_SECONDS));
        *cached = Some(CachedToken {
            value: response.access_token.clone(),
            refresh_after,
        });
        Ok(response.access_token)
    }

    async fn usable_cached_token(&self) -> Option<String> {
        self.token
            .read()
            .await
            .as_ref()
            .filter(|token| Instant::now() < token.refresh_after)
            .map(|token| token.value.clone())
    }

    fn build_stk_request(&self, phone_number: &str, amount: i64) -> StkPushRequest {
        let timestamp = (Utc::now() + ChronoDuration::hours(3))
            .format("%Y%m%d%H%M%S")
            .to_string();
        let password = STANDARD.encode(format!(
            "{}{}{}",
            self.config.shortcode, self.config.passkey, timestamp
        ));

        StkPushRequest {
            business_short_code: self.config.shortcode.clone(),
            password,
            timestamp,
            transaction_type: self.config.transaction_type.as_str().to_owned(),
            amount: amount.to_string(),
            party_a: phone_number.to_owned(),
            party_b: self.config.shortcode.clone(),
            phone_number: phone_number.to_owned(),
            call_back_url: self.config.callback_url.clone(),
            account_reference: "ROSCAs".to_owned(),
            transaction_desc: "Chama contribution".to_owned(),
        }
    }
}

impl ExpiresIn {
    fn seconds(self) -> Result<u64> {
        match self {
            Self::Seconds(seconds) => Ok(seconds),
            Self::String(seconds) => seconds
                .parse()
                .context("Daraja expires_in was not a number of seconds"),
        }
    }
}

fn optional_env(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn require_env_value(name: &str, value: Option<String>) -> Result<String> {
    value.ok_or_else(|| anyhow!("{name} is required when configuring Daraja"))
}

fn validate_https_url(name: &str, value: &str) -> Result<()> {
    let url = Url::parse(value).with_context(|| format!("{name} must be a valid URL"))?;
    if url.scheme() != "https" || url.host_str().is_none() {
        bail!("{name} must be an HTTPS URL");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server};

    fn test_config(base_url: String) -> DarajaConfig {
        DarajaConfig {
            base_url,
            consumer_key: "test-key".to_owned(),
            consumer_secret: "test-secret".to_owned(),
            shortcode: "174379".to_owned(),
            passkey: "test-passkey".to_owned(),
            callback_url: "https://example.test/api/v1/mpesa/callback".to_owned(),
            transaction_type: TransactionType::CustomerPayBillOnline,
        }
    }

    #[tokio::test]
    async fn fetches_oauth_token_once_and_sends_integer_amount_string() {
        let mut server = Server::new_async().await;
        let oauth = server
            .mock("GET", "/oauth/v1/generate")
            .match_query(Matcher::UrlEncoded(
                "grant_type".to_owned(),
                "client_credentials".to_owned(),
            ))
            .match_header("authorization", Matcher::Regex("^Basic .+".to_owned()))
            .with_status(200)
            .with_body(r#"{"access_token":"test-token","expires_in":"3599"}"#)
            .expect(1)
            .create_async()
            .await;
        let stk = server
            .mock("POST", "/mpesa/stkpush/v1/processrequest")
            .match_header("authorization", "Bearer test-token")
            .match_body(Matcher::Regex(r#""Amount":"100""#.to_owned()))
            .with_status(200)
            .with_body(
                r#"{"MerchantRequestID":"merchant-1","CheckoutRequestID":"checkout-1","ResponseCode":"0","ResponseDescription":"Success"}"#,
            )
            .expect(1)
            .create_async()
            .await;
        let client = DarajaClient::new(test_config(server.url())).unwrap();

        let first = client.access_token().await.unwrap();
        let second = client.access_token().await.unwrap();
        assert_eq!(first, "test-token");
        assert_eq!(second, first);

        let result = client.initiate_stk_push("254712345678", 100).await.unwrap();
        assert_eq!(result.merchant_request_id, "merchant-1");
        assert_eq!(result.checkout_request_id, "checkout-1");

        oauth.assert_async().await;
        stk.assert_async().await;
    }

    #[test]
    fn accepts_numeric_or_string_token_expiry() {
        let string_expiry: OAuthResponse =
            serde_json::from_str(r#"{"access_token":"token","expires_in":"3599"}"#).unwrap();
        let numeric_expiry: OAuthResponse =
            serde_json::from_str(r#"{"access_token":"token","expires_in":3599}"#).unwrap();

        assert_eq!(string_expiry.expires_in.seconds().unwrap(), 3599);
        assert_eq!(numeric_expiry.expires_in.seconds().unwrap(), 3599);
    }
}
