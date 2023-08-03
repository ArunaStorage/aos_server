use super::grpc_query_handler::GrpcQueryHandler;
use crate::{
    database::database::Database,
    structs::{Object, ObjectLocation, User},
};
use ahash::RandomState;
use anyhow::anyhow;
use anyhow::Result;
use aruna_rust_api::api::storage::services::v2::Pubkey;
use dashmap::{DashMap, DashSet};
use diesel_ulid::DieselUlid;
use s3s::auth::SecretKey;
use std::sync::{Arc, Mutex, RwLock};

pub struct Cache {
    // Map with SecretKey as key and User as value
    pub users: DashMap<String, User, RandomState>,
    // Map with ObjectId as key and Object as value
    pub objects: DashMap<DieselUlid, (Object, ObjectLocation), RandomState>,
    // Maps with path as key and set of ObjectIds as value
    pub paths: DashMap<String, DashSet<DieselUlid>, RandomState>,
    // Persistence layer
    pub persistence: Option<Arc<Database>>,

    pub notifications: Option<GrpcQueryHandler>,
}

impl Cache {
    pub async fn new(
        notifications_url: Option<impl Into<String>>,
        with_persistence: bool,
    ) -> Result<Arc<RwLock<Self>>> {
        let persistence = if with_persistence {
            None
        } else {
            Some(Arc::new(Database::new()?))
        };
        let cache = Arc::new(RwLock::new(Cache {
            users: DashMap::default(),
            objects: DashMap::default(),
            paths: DashMap::default(),
            persistence,
            notifications: None,
        }));
        let notifications = match notifications_url {
            Some(s) => Some(GrpcQueryHandler::new(s, cache.clone()).await?),
            None => None,
        };
        cache.write().unwrap().notifications = notifications;
        Ok(cache)
    }

    /// Requests a secret key from the cache
    pub fn get_secret(&self, access_key: &str) -> Result<SecretKey> {
        Ok(SecretKey::from(
            self.users
                .get(access_key)
                .ok_or_else(|| anyhow!("User not found"))?
                .secret
                .as_ref(),
        ))
    }

    pub fn set_pubkeys(&self, pks: Vec<Pubkey>) -> Result<()> {
        Ok(())
    }
}
