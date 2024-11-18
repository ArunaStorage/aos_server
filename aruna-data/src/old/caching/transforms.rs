use crate::structs::DbPermissionLevel;
use anyhow::{anyhow, Result};
use aruna_rust_api::api::storage::models::v2::permission::ResourceId;
use aruna_rust_api::api::storage::models::v2::Permission;
use aruna_rust_api::api::storage::models::v2::User;
use diesel_ulid::DieselUlid;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::error;

pub trait ExtractAccessKeyPermissions {
    #[allow(dead_code)]
    fn extract_access_key_permissions(
        &self,
    ) -> Result<Vec<(String, HashMap<DieselUlid, DbPermissionLevel>)>>;
}

pub trait GetId {
    fn get_id(&self) -> Result<DieselUlid>;
}

impl GetId for ResourceId {
    #[tracing::instrument(level = "trace", skip(self))]
    fn get_id(&self) -> Result<DieselUlid> {
        match self {
            ResourceId::ProjectId(a)
            | ResourceId::CollectionId(a)
            | ResourceId::DatasetId(a)
            | ResourceId::ObjectId(a) => Ok(DieselUlid::from_str(a).inspect_err(|&e| {
                error!(error = ?e, msg = e.to_string());
            })?),
        }
    }
}

pub trait IntoHashMap {
    fn into_hash_map(self) -> Result<HashMap<DieselUlid, DbPermissionLevel>>;
}

impl IntoHashMap for Permission {
    #[tracing::instrument(level = "trace", skip(self))]
    fn into_hash_map(self) -> Result<HashMap<DieselUlid, DbPermissionLevel>> {
        let mut map = HashMap::new();
        map.insert(
            self.resource_id
                .clone()
                .ok_or_else(|| {
                    error!(error = "Unknown resource");
                    anyhow!("Unknown resource")
                })?
                .get_id()?,
            DbPermissionLevel::from(self.permission_level()),
        );
        Ok(map)
    }
}

impl ExtractAccessKeyPermissions for User {
    #[tracing::instrument(level = "trace", skip(self))]
    fn extract_access_key_permissions(
        &self,
    ) -> Result<Vec<(String, HashMap<DieselUlid, DbPermissionLevel>)>> {
        let personal_permissions = HashMap::from_iter(
            self.attributes
                .clone()
                .ok_or_else(|| {
                    error!(error = "Unknown attributes");
                    anyhow!("Unknown attributes")
                })?
                .personal_permissions
                .iter()
                .map(|p| {
                    Ok((
                        p.resource_id
                            .clone()
                            .ok_or_else(|| {
                                error!(error = "Unknown resource");
                                anyhow!("Unknown resource")
                            })?
                            .get_id()?,
                        DbPermissionLevel::from(p.permission_level()),
                    ))
                })
                .collect::<Result<Vec<(DieselUlid, DbPermissionLevel)>>>()?,
        );

        let mut a_key_perm = vec![(self.id.clone(), personal_permissions.clone())];

        for t in self
            .attributes
            .clone()
            .ok_or_else(|| {
                error!(error = "Unknown attributes");
                anyhow!("Unknown attributes")
            })?
            .tokens
        {
            match t.permission {
                Some(p) => a_key_perm.push((t.id, p.into_hash_map()?)),
                None => a_key_perm.push((t.id, personal_permissions.clone())),
            }
        }
        Ok(a_key_perm)
    }
}
