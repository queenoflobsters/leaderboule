use dioxus::{logger::tracing, prelude::*};
use serde::Deserialize;
use std::{sync::{Arc, OnceLock}, time::Duration};
use tokio::{
    sync::{RwLock},
    time::Instant,
};

const HELLOASSO_OAUTH_TOKEN_URL: &'static str = "https://api.helloasso.com/oauth2/token";
const HELLOASSO_API_URL: &'static str = "https://api.helloasso.com/v5";
const ASSOCIATION_SLUG: &'static str = "petanqu-insa-club";
// WARNING TODO WARNING change this form slug
const ASSO_FORM_SLUG: &'static str = "adhesion-2025-2026-petanqu-insa-club";
static HELLOASSO_CLIENT: OnceLock<HelloassoClient> =OnceLock::new();


pub fn get() -> &'static HelloassoClient {
    HELLOASSO_CLIENT.get_or_init(|| {
        let client_id = std::env::var("HELLOASSO_CLIENT_ID").unwrap();
        let client_secret = std::env::var("HELLOASSO_CLIENT_SECRET").unwrap();
        HelloassoClient::new(client_id, client_secret)
    })
}

#[derive(Clone)]
pub struct HelloassoClient {
    client: reqwest::Client,
    client_id: String,
    client_secret: String,
    tokens: Arc<RwLock<Option<HelloassoTokens>>>,
}


impl HelloassoClient {

    pub async fn is_adherent(&self, email: &str) -> Result<bool, ServerFnError> {
        // TODO for faster login time, implement a helloasso hook to listen to new memberships
        // store them in a new database table a create a new account for them at every adhesion
        let access_token = self.get_access_token().await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let api_url = format!("{}/organizations/{}/forms/Membership/{}/items", HELLOASSO_API_URL, ASSOCIATION_SLUG, ASSO_FORM_SLUG);

        let items: Vec<MemberItem> = get().client.get(&api_url)
            .bearer_auth(access_token)
            .query(&[
                ("userSearchKey", email),
                ("withDetails", "false"),
                ("itemStates", "Processed")
            ])
            .send()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .json()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        tracing::info!("{:?}", items);
        Ok(true)
    }

    fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            client_id,
            client_secret,
            tokens: Arc::new(RwLock::new(None)),
        }
    }

    async fn get_access_token(&self) -> Result<String, reqwest::Error> {
        // added time for security
        let expire_instant = Instant::now() + Duration::from_secs(60);

        let token_read_guard = self.tokens.read().await.clone();

        match token_read_guard {
            Some(ref tokens) if tokens.expires_at > expire_instant => {
                Ok(tokens.access_token.clone())
            }
            Some(ref tokens) => self.request_token_refresh(tokens).await,
            None => self.request_access_token().await,
        }
    }

    async fn request_token_refresh(
        &self,
        tokens: &HelloassoTokens,
    ) -> Result<String, reqwest::Error> {
        let res = self
            .client
            .post(HELLOASSO_OAUTH_TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &tokens.refresh_token),
            ])
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;

        let mut token_write_guard = self.tokens.write().await;
        *token_write_guard = Some(HelloassoTokens {
            access_token: res.access_token.clone(),
            refresh_token: res.refresh_token,
            expires_at: Instant::now() + Duration::from_secs(res.expires_in),
        });
        Ok(res.access_token)
    }

    async fn request_access_token(&self) -> Result<String, reqwest::Error> {        
        let res = self
            .client
            .post(HELLOASSO_OAUTH_TOKEN_URL)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", &self.client_id),
                ("client_secret", &self.client_secret),
            ])
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;

        let mut token_write_guard = self.tokens.write().await;
        *token_write_guard = Some(HelloassoTokens {
            access_token: res.access_token.clone(),
            refresh_token: res.refresh_token,
            expires_at: Instant::now() + Duration::from_secs(res.expires_in),
        });
        Ok(res.access_token)
    }

}

#[derive(Deserialize, Debug)]
struct MemberItem {
    pub user: UserInfo,
}

#[derive(Deserialize, Debug, Clone)]
struct UserInfo {
    #[serde(rename = "firstName")]
    first_name: Option<String>,

    #[serde(rename = "lastName")]
    last_name: Option<String>,

    email: Option<String>
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

#[derive(Clone)]
struct HelloassoTokens {
    access_token: String,
    refresh_token: String,
    expires_at: Instant,
}

