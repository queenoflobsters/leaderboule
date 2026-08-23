/// COMMENT ÇA FONCTIONNE
///     Pour faire un call à l'API Helloasso, il faut un `access_token`
///     Pour avoir un `access_token` il faut un Client ID et un Client Secret, qui peuvent être récupérés sur le site d'Helloasso
///     La requête avec le user_id et user_secret retourne un `access_token` EEEETT un `refresh_token`
///     Pourquoi ? Parce que l'`access_token` n'est valable que pour 30 minutes, après il faut en redemander un
///     MAIS tu peux pas en redemander un simplement avec ton user_id et ton user_secret (enfin si mais c'est pas recommander)
///     Il faut faire un requête pour un NOUVEAU `access_token` avec le `refresh_token` qui lui est valable 30 jours
///     Sauf que quand tu fais un requête pour un nouveau `access_token` ça te donne AUSSI un nouveau `refresh_token`
///     Donc ils sont rafraichis en même temps en fait
/// VOILÀ

use dioxus::prelude::*;
use serde::Deserialize;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::{sync::RwLock, time::Instant};

const HELLOASSO_OAUTH_TOKEN_URL: &'static str = "https://api.helloasso.com/oauth2/token";
const HELLOASSO_API_URL: &'static str = "https://api.helloasso.com/v5";
const ASSOCIATION_SLUG: &'static str = "petanqu-insa-club";
// WARNING TODO WARNING change this form slug
const ASSO_FORM_SLUG: &'static str = "adhesion-2025-2026-petanqu-insa-club";
static HELLOASSO_CLIENT: OnceLock<HelloassoClient> = OnceLock::new();

/// Initialise la connexion Helloasso
pub fn get() -> &'static HelloassoClient {
    HELLOASSO_CLIENT.get_or_init(|| {
        let client_id = std::env::var("HELLOASSO_CLIENT_ID").unwrap();
        let client_secret = std::env::var("HELLOASSO_CLIENT_SECRET").unwrap();
        HelloassoClient::new(client_id, client_secret)
    })
}

/// Communication avec l'API Helloasso
/// Réponse à une demande de nouveaux Token
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
}

/// Les tokens Helloasso
#[derive(Clone)]
struct HelloassoTokens {
    access_token: String,
    refresh_token: String,
    expires_at: Instant,
}

/// Singleton correspondant à une connexion avec Helloasso
#[derive(Clone)]
pub struct HelloassoClient {
    client: reqwest::Client,
    client_id: String,
    client_secret: String,
    tokens: Arc<RwLock<Option<HelloassoTokens>>>,
}

/// Retourne les information d'un utilisateur si il est présent dans la base de donnée
pub async fn get_adherent(email: &str) -> Result<Option<PayerInfo>, ServerFnError> {
    // TODO for faster login time, implement a helloasso hook to listen to new memberships
    // store them in a new database table a create a new account for them at every adhesion
    let access_token = get()
        .get_access_token()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let api_url = format!(
        "{}/organizations/{}/forms/Membership/{}/items",
        HELLOASSO_API_URL, ASSOCIATION_SLUG, ASSO_FORM_SLUG
    );

    debug!("HA : requesting payers list from Helloasso");

    let response = get()
        .client
        .get(&api_url)
        .bearer_auth(access_token)
        .query(&[
            ("userSearchKey", email),
            ("withDetails", "false"),
            ("itemStates", "Processed"),
        ])
        .send()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .json::<HelloassoResponse>()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    debug!("HA : response : {:?}", response);
    let agg = response.aggregate_payers(email);
    debug!("HA : aggregate_payers : {:?}", agg);
    Ok(agg)
}

impl HelloassoClient {
    fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            client_id,
            client_secret,
            tokens: Arc::new(RwLock::new(None)),
        }
    }

    /// Retourne le token d'accès à l'API
    async fn get_access_token(&self) -> Result<String, reqwest::Error> {
        debug!("HA : trying to read access_token");

        // added time for security
        let expire_instant = Instant::now() + Duration::from_secs(60);

        let token_read_guard = self.tokens.write().await.clone();

        match token_read_guard {
            Some(ref tokens) if tokens.expires_at > expire_instant => {
                debug!("HA : found access_token, returning");
                Ok(tokens.access_token.clone())
            }
            Some(ref tokens) => {
                debug!("HA : access_token expired, requesting token refresh");
                self.request_token_refresh(tokens).await
            }
            None => {
                debug!("HA : no access_token, requesting full access_token and refresh_token");
                self.request_access_token().await
            }
        }
    }

    /// Fais une requête pour rafraîchir les tokens
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
        debug!("HA : successfully refreshed tokens");
        Ok(res.access_token)
    }

    /// Fais une requête pour le token d'accès
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
        debug!("HA : successfully acquired access and refresh tokens");
        Ok(res.access_token)
    }
}

/// reqwest .json() deserialization
#[derive(Deserialize, Debug)]
struct HelloassoResponse {
    data: Vec<HelloassoResponseDataItem>,
}

/// reqwest .json() deserialization
#[derive(Deserialize, Debug)]
struct HelloassoResponseDataItem {
    payer: PayerInfo,
}

/// reqwest .json() deserialization
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PayerInfo {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
}

impl HelloassoResponse {
    /// Consomme la réponse et la transforme en information sur le payeur
    fn aggregate_payers(self, email: &str) -> Option<PayerInfo> {
        let mut agg = PayerInfo::default();

        for HelloassoResponseDataItem { payer } in self.data {
            if let Some(payer_email) = &payer.email {
                if payer_email == email {
                    agg.email = payer.email;
                } else {
                    continue;
                }
            }
            agg.first_name = agg.first_name.or(payer.first_name);
            agg.last_name = agg.last_name.or(payer.last_name);
        }

        if agg.email.is_some() {
            Some(agg)
        } else {
            None
        }
    }
}

