use anyhow::{anyhow, bail, Result};
use chrono::{NaiveDateTime, Utc};
use jsonwebtoken::{decode_header, jwk::JwkSet, DecodingKey};

use crate::database::dsls::pub_key_dsl::PubKey;

use super::token_handler::ArunaTokenClaims;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssuerType {
    ARUNA,
    DATAPROXY,
    OIDC,
}

pub struct Issuer {
    pub issuer_name: String,
    pub pubkey_endpoint: Option<String>,
    pub decoding_keys: Vec<(String, DecodingKey)>,
    pub last_updated: NaiveDateTime,
    pub audiences: Vec<String>,
    pub issuer_type: IssuerType,
}

impl Issuer {
    pub async fn new_with_endpoint(
        issuer_name: String,
        pubkey_endpoint: String,
        audiences: Vec<String>,
    ) -> Result<Self> {
        let (decoding_keys, last_updated) = Self::fetch_jwks(&pubkey_endpoint).await?;
        Ok(Self {
            issuer_name,
            pubkey_endpoint: Some(pubkey_endpoint),
            decoding_keys,
            last_updated,
            audiences,
            issuer_type: IssuerType::OIDC,
        })
    }

    pub async fn new_with_keys(
        issuer_name: String,
        decoding_keys: Vec<(String, DecodingKey)>,
        audiences: Vec<String>,
        issuer_type: IssuerType,
    ) -> Result<Self> {
        Ok(Self {
            issuer_name,
            pubkey_endpoint: None,
            decoding_keys,
            last_updated: Utc::now().naive_utc(),
            audiences,
            issuer_type,
        })
    }

    pub async fn fetch_jwks(endpoint: &str) -> Result<(Vec<(String, DecodingKey)>, NaiveDateTime)> {
        let client = reqwest::Client::new();
        let res = client.get(endpoint).send().await?;
        let jwks: JwkSet = res.json().await?;

        Ok((
            jwks.keys
                .iter()
                .filter_map(|jwk| {
                    let key = DecodingKey::from_jwk(jwk).ok()?;
                    Some((jwk.common.clone().key_id?, key))
                })
                .collect::<Vec<_>>(),
            Utc::now().naive_utc(),
        ))
    }

    pub async fn refresh_jwks(&mut self) -> Result<()> {
        if self.last_updated + chrono::Duration::minutes(5) > Utc::now().naive_utc() {
            bail!("JWKS was updated less than 5 minutes ago");
        }

        if self.issuer_type != IssuerType::OIDC {
            bail!("Only OIDC issuers can refresh JWKS");
        }
        let (decodings_keys, last_updated) = Self::fetch_jwks(
            &self
                .pubkey_endpoint
                .as_ref()
                .ok_or_else(|| anyhow!("Invalid endpoint type"))?,
        )
        .await?;
        self.decoding_keys = decodings_keys;
        self.last_updated = last_updated;
        Ok(())
    }

    pub fn find(&self, kid: &str) -> Option<&DecodingKey> {
        self.decoding_keys
            .iter()
            .find(|(key_id, _)| key_id == kid)
            .map(|(_, key)| key)
    }

    pub async fn check_token(&mut self, token: &str) -> Result<(String, ArunaTokenClaims)> {
        let kid = decode_header(token)?
            .kid
            .ok_or_else(|| anyhow!("No kid in header"))?;
        match self.find(&kid) {
            Some(decoding_key) => Ok((
                kid,
                Self::get_validate_claims(token, decoding_key, &self.audiences)?,
            )),
            None => {
                self.refresh_jwks().await?;
                if let Some(decoding_key) = self.find(&kid) {
                    Ok((
                        kid,
                        Self::get_validate_claims(token, &decoding_key, &self.audiences)?,
                    ))
                } else {
                    bail!("No matching key found")
                }
            }
        }
    }

    pub fn get_validate_claims(
        token: &str,
        decoding_key: &DecodingKey,
        audiences: &[String],
    ) -> Result<ArunaTokenClaims> {
        // if !jwk.is_supported() {
        //     return Err(anyhow::anyhow!("Unsupported algorithm"));
        // }
        let header = decode_header(token)?;
        let alg = header.alg;
        let mut validation = jsonwebtoken::Validation::new(alg);
        validation.set_audience(audiences);
        let tokendata =
            jsonwebtoken::decode::<ArunaTokenClaims>(token, &decoding_key, &validation)?;
        Ok(tokendata.claims)
    }
}

impl From<Vec<PubKey>> for Vec<Issuer> {
    fn from(value: Vec<PubKey>) -> Self {}
}
