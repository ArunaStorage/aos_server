use anyhow::anyhow;
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, ToSql, FromSql, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum ObjectStatus {
    INITIALIZING,
    VALIDATING,
    AVAILABLE,
    ERROR,
    DELETED,
}

impl TryFrom<&[u8]> for ObjectStatus {
    type Error = Box<dyn Error + Sync + Send>;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match String::from_utf8_lossy(value).as_ref() {
            "INIALIZING" => Ok(ObjectStatus::INITIALIZING),
            "VALIDATING" => Ok(ObjectStatus::VALIDATING),
            "AVAILABLE" => Ok(ObjectStatus::AVAILABLE),
            "ERROR" => Ok(ObjectStatus::ERROR),
            "DELETED" => Ok(ObjectStatus::DELETED),
            _ => Err(anyhow!("Unknown type").into()),
        }
    }
}

#[derive(Debug, ToSql, FromSql, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum DataClass {
    PUBLIC,
    PRIVATE,
    CONFIDENTIAL,
}

#[derive(Debug, ToSql, FromSql, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum ObjectType {
    PROJECT,
    COLLECTION,
    DATASET,
    OBJECT,
}

#[derive(Serialize, Deserialize, Debug, ToSql, FromSql, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum PermissionLevels {
    DENY,
    NONE,
    READ,
    APPEND,
    WRITE,
    ADMIN,
}

impl TryFrom<i32> for PermissionLevels {
    type Error = Box<dyn Error + Sync + Send>;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(PermissionLevels::DENY),
            2 => Ok(PermissionLevels::NONE),
            3 => Ok(PermissionLevels::READ),
            4 => Ok(PermissionLevels::APPEND),
            5 => Ok(PermissionLevels::WRITE),
            6 => Ok(PermissionLevels::ADMIN),
            _ => Err(anyhow!("Unknown permission level").into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, PartialOrd)]
pub enum DataProxyFeature {
    #[default]
    PROXY,
    INTERNAL,
    BUNDLER,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, FromSql)]
pub enum EndpointStatus {
    INITIALIZING,
    AVAILABLE,
    DEGRADED,
    UNAVAILABLE,
    MAINTENANCE,
}
