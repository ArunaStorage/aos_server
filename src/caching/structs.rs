use crate::database::dsls::object_dsl::ObjectWithRelations;
use crate::database::dsls::pub_key_dsl::PubKey;
use anyhow::Result;
use diesel_ulid::DieselUlid;
use jsonwebtoken::DecodingKey;

#[derive(Clone)]
pub enum PubKeyEnum {
    DataProxy((String, DecodingKey, DieselUlid)), // DataProxy((Raw Key String, DecodingKey, Endpoint ID))
    Server((String, DecodingKey)), // Server((Key String, DecodingKey)) + ArunaServer ID ?
}

impl PubKeyEnum {
    pub fn get_key_string(&self) -> String {
        match self {
            PubKeyEnum::DataProxy((k, _, _)) => k.to_string(),
            PubKeyEnum::Server((k, _)) => k.to_string(),
        }
    }

    pub fn get_name(&self) -> String {
        match self {
            PubKeyEnum::DataProxy((_, _, n)) => n.to_string(),
            PubKeyEnum::Server((_, _)) => "".to_string(),
        }
    }
}

impl TryFrom<PubKey> for PubKeyEnum {
    type Error = anyhow::Error;
    fn try_from(pk: PubKey) -> Result<Self> {
        let public_pem = format!(
            "-----BEGIN PUBLIC KEY-----{}-----END PUBLIC KEY-----",
            &pk.pubkey
        );
        let decoding_key = DecodingKey::from_ed_pem(public_pem.as_bytes())?;

        Ok(match pk.proxy {
            Some(proxy) => PubKeyEnum::DataProxy((pk.pubkey.to_string(), decoding_key, proxy)),
            None => PubKeyEnum::Server((pk.pubkey.to_string(), decoding_key)),
        })
    }
}

impl ObjectWithRelations {
    pub fn get_children(&self) -> Vec<DieselUlid> {
        self.outbound_belongs_to
            .0
            .iter()
            .map(|x| *x.key())
            .collect::<Vec<_>>()
    }

    pub fn get_parents(&self) -> Vec<DieselUlid> {
        self.inbound_belongs_to
            .0
            .iter()
            .map(|x| *x.key())
            .collect::<Vec<_>>()
    }
}
