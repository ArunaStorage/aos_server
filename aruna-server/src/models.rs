use chrono::{DateTime, NaiveDateTime, Utc};
use jsonwebtoken::DecodingKey;
use obkv::KvReaderU16;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::fmt::Display;
use ulid::Ulid;
use utoipa::{IntoParams, ToSchema};

use crate::{
    constants::relation_types::*,
    error::ArunaError,
    storage::obkv_ext::{FieldIterator, ParseError},
};

pub type EdgeType = u32;

// Constants for the models

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub enum ResourceVariant {
    Project,
    Folder,
    Object,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum NodeVariant {
    ResourceProject = 0,
    ResourceFolder = 1,
    ResourceObject = 2,
    User = 3,
    Token = 4,
    ServiceAccount = 5,
    Group = 6,
    Realm = 7,
}

impl TryFrom<serde_json::Number> for NodeVariant {
    type Error = ArunaError;

    fn try_from(value: serde_json::Number) -> Result<Self, Self::Error> {
        value.as_u64().map_or_else(
            || {
                Err(ArunaError::ConversionError {
                    from: "serde_json::Number".to_string(),
                    to: "models::NodeVariant".to_string(),
                })
            },
            |v| {
                Ok(match v {
                    0 => NodeVariant::ResourceProject,
                    1 => NodeVariant::ResourceFolder,
                    2 => NodeVariant::ResourceObject,
                    3 => NodeVariant::User,
                    4 => NodeVariant::Token,
                    5 => NodeVariant::ServiceAccount,
                    6 => NodeVariant::Group,
                    7 => NodeVariant::Realm,
                    _ => {
                        return Err(ArunaError::ConversionError {
                            from: format!("{}u64", v),
                            to: "models::NodeVariant".to_string(),
                        })
                    }
                })
            },
        )
    }
}

pub trait Node<'a>:
    TryFrom<&'a KvReaderU16<'a>, Error = ParseError>
    + TryInto<serde_json::Map<String, Value>, Error = ArunaError>
{
}

// Helper fuction to convert a struct to serde_json::Map<String, Value>
pub fn into_serde_json_map<T: Serialize>(
    value: T,
    variant: NodeVariant,
) -> Result<serde_json::Map<String, Value>, ArunaError> {
    let value = serde_json::to_value(value).map_err(|e| {
        tracing::error!(?e, "Error converting to serde_json::Value");
        ArunaError::ConversionError {
            from: "models::Node".to_string(),
            to: "serde_json::Map<String, Value>".to_string(),
        }
    })?;
    match value {
        Value::Object(mut map) => {
            map.insert(
                "variant".to_string(),
                Value::Number(Number::from(variant as u64)),
            );
            Ok(map)
        }
        _ => Err(ArunaError::ConversionError {
            from: "models::Node".to_string(),
            to: "serde_json::Map<String, Value>".to_string(),
        }),
    }
}

impl Node<'_> for Resource {}

impl TryFrom<Resource> for serde_json::Map<String, Value> {
    type Error = ArunaError;
    fn try_from(r: Resource) -> Result<Self, Self::Error> {
        Ok(match r.variant {
            ResourceVariant::Project => into_serde_json_map(r, NodeVariant::ResourceProject)?,
            ResourceVariant::Folder => into_serde_json_map(r, NodeVariant::ResourceFolder)?,
            ResourceVariant::Object => into_serde_json_map(r, NodeVariant::ResourceObject)?,
        })
    }
}

// Implement TryFrom for Resource
impl<'a> TryFrom<&KvReaderU16<'a>> for Resource {
    type Error = ParseError;

    fn try_from(obkv: &KvReaderU16<'a>) -> Result<Self, Self::Error> {
        let mut obkv = FieldIterator::new(obkv);
        Ok(Resource {
            id: obkv.get_required_field(0)?,
            variant: obkv.get_required_field(1)?,
            name: obkv.get_required_field(2)?,
            description: obkv.get_field(3)?,
            revision: 0,
            labels: obkv.get_field(4)?,
            identifiers: obkv.get_field(5)?,
            content_len: obkv.get_field(6)?,
            count: obkv.get_field(7)?,
            visibility: obkv.get_field(8)?,
            created_at: obkv.get_field(9)?,
            last_modified: obkv.get_field(10)?,
            authors: obkv.get_field(11)?,
            locked: obkv.get_field(12)?,
            license_tag: obkv.get_field(13)?,
            hashes: obkv.get_field(14)?,
            location: obkv.get_field(15)?,
            title: obkv.get_field(22)?,
        })
    }
}

impl Node<'_> for User {}

impl TryFrom<User> for serde_json::Map<String, Value> {
    type Error = ArunaError;
    fn try_from(u: User) -> Result<Self, Self::Error> {
        into_serde_json_map(u, NodeVariant::User)
    }
}

// Implement TryFrom for User
impl<'a> TryFrom<&KvReaderU16<'a>> for User {
    type Error = ParseError;

    fn try_from(obkv: &KvReaderU16<'a>) -> Result<Self, Self::Error> {
        let mut obkv = FieldIterator::new(obkv);
        // Get the required id
        let id: Ulid = obkv.get_required_field(0)?;
        // Get and double check the variant
        let variant: u8 = obkv.get_required_field(1)?;
        if variant != NodeVariant::User as u8 {
            return Err(ParseError(format!("Invalid variant for User: {}", variant)));
        }
        Ok(User {
            id,
            identifiers: obkv.get_field(5)?,
            first_name: obkv.get_required_field(18)?,
            last_name: obkv.get_required_field(19)?,
            email: obkv.get_required_field(20)?,
            global_admin: obkv.get_field(21)?,
        })
    }
}

impl Node<'_> for Token {}

impl TryFrom<Token> for serde_json::Map<String, Value> {
    type Error = ArunaError;
    fn try_from(t: Token) -> Result<Self, Self::Error> {
        into_serde_json_map(t, NodeVariant::Token)
    }
}

// Implement TryFrom for Token
impl<'a> TryFrom<&KvReaderU16<'a>> for Token {
    type Error = ParseError;

    fn try_from(obkv: &KvReaderU16<'a>) -> Result<Self, Self::Error> {
        let mut obkv = FieldIterator::new(obkv);
        // Get the required id
        let id: Ulid = obkv.get_required_field(0)?;
        // Get and double check the variant
        let variant: u8 = obkv.get_required_field(1)?;
        if variant != NodeVariant::Token as u8 {
            return Err(ParseError(format!("Invalid variant for User: {}", variant)));
        }
        Ok(Token {
            id,
            name: obkv.get_field(2)?,
            expires_at: obkv.get_field(17)?,
        })
    }
}

impl Node<'_> for ServiceAccount {}

impl TryFrom<ServiceAccount> for serde_json::Map<String, Value> {
    type Error = ArunaError;
    fn try_from(sa: ServiceAccount) -> Result<Self, Self::Error> {
        into_serde_json_map(sa, NodeVariant::ServiceAccount)
    }
}

// Implement TryFrom for ServiceAccount
impl<'a> TryFrom<&KvReaderU16<'a>> for ServiceAccount {
    type Error = ParseError;

    fn try_from(obkv: &KvReaderU16<'a>) -> Result<Self, Self::Error> {
        let mut obkv = FieldIterator::new(obkv);
        // Get the required id
        let id: Ulid = obkv.get_required_field(0)?;
        // Get and double check the variant
        let variant: u8 = obkv.get_required_field(1)?;
        if variant != NodeVariant::ServiceAccount as u8 {
            return Err(ParseError(format!("Invalid variant for User: {}", variant)));
        }
        Ok(ServiceAccount {
            id,
            name: obkv.get_field(2)?,
        })
    }
}

impl Node<'_> for Group {}

impl TryFrom<Group> for serde_json::Map<String, Value> {
    type Error = ArunaError;
    fn try_from(g: Group) -> Result<Self, Self::Error> {
        into_serde_json_map(g, NodeVariant::Group)
    }
}

// Implement TryFrom for Group
impl<'a> TryFrom<&KvReaderU16<'a>> for Group {
    type Error = ParseError;

    fn try_from(obkv: &KvReaderU16<'a>) -> Result<Self, Self::Error> {
        let mut obkv = FieldIterator::new(obkv);
        // Get the required id
        let id: Ulid = obkv.get_required_field(0)?;
        // Get and double check the variant
        let variant: u8 = obkv.get_required_field(1)?;
        if variant != NodeVariant::Group as u8 {
            return Err(ParseError(format!("Invalid variant for User: {}", variant)));
        }
        Ok(Group {
            id,
            name: obkv.get_required_field(2)?,
            description: obkv.get_field(3)?,
        })
    }
}

impl Node<'_> for Realm {}

impl TryFrom<Realm> for serde_json::Map<String, Value> {
    type Error = ArunaError;
    fn try_from(r: Realm) -> Result<Self, Self::Error> {
        into_serde_json_map(r, NodeVariant::Realm)
    }
}

// Implement TryFrom for Group
impl<'a> TryFrom<&KvReaderU16<'a>> for Realm {
    type Error = ParseError;

    fn try_from(obkv: &KvReaderU16<'a>) -> Result<Self, Self::Error> {
        let mut obkv = FieldIterator::new(obkv);
        // Get the required id
        let id: Ulid = obkv.get_required_field(0)?;
        // Get and double check the variant
        let variant: u8 = obkv.get_required_field(1)?;
        if variant != NodeVariant::Realm as u8 {
            return Err(ParseError(format!("Invalid variant for User: {}", variant)));
        }
        Ok(Realm {
            id,
            name: obkv.get_required_field(2)?,
            description: obkv.get_field(3)?,
            tag: obkv.get_field(22)?,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum NodeVariantIdx {
    Resource(u32),
    User(u32),
    ServiceAccount(u32),
    Token(u32),
    Group(u32),
    Realm(u32),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub locked: bool,
}

// TODO: Decide how hooks are going to be implemented
pub enum HookExecutionState {
    Pending,
    Running,
    Finished,
    Error,
}

// TODO: Decide how hooks are going to be implemented
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct HookRunStatus {
    hook_id: Ulid,
    run_id: Ulid,
    revision: u64,
    status: String,
    last_updated: DateTime<Utc>,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, Default, ToSchema,
)]
pub enum VisibilityClass {
    Public,
    PublicMetadata,
    #[default]
    Private,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Author {
    pub id: Ulid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub identifier: String,
}

// ArunaGraph Nodes
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Realm {
    pub id: Ulid,
    pub tag: String, // -> Region
    pub name: String,
    pub description: String,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Group {
    pub id: Ulid,
    pub name: String,
    pub description: String,
    // TODO: OIDC mapping ?
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Token {
    pub id: Ulid,
    pub name: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct TokenWithPermission {
    pub id: Ulid,
    pub name: String,
    pub expires_at: DateTime<Utc>,
    pub permission: Permission,
    pub resource_id: Ulid,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema, Default,
)]
pub struct User {
    pub id: Ulid,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub identifiers: String, // TODO: Vec<String>?
    /// TODO: OIDC mapping ?
    pub global_admin: bool,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct ServiceAccount {
    pub id: Ulid,
    pub name: String,
    // TODO: More fields?
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Resource {
    pub id: Ulid,
    pub name: String,
    pub title: String,
    pub description: String,
    pub revision: u64, // This should not be part of the index
    pub variant: ResourceVariant,
    pub labels: Vec<KeyValue>,
    //pub hook_status: Vec<KeyValue>, // TODO: Hooks ? Not part of the index
    pub identifiers: Vec<String>,
    pub content_len: u64,
    pub count: u64,
    pub visibility: VisibilityClass,
    pub created_at: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub authors: Vec<Author>,
    pub license_tag: String,
    pub locked: bool,
    // TODO:
    pub location: Vec<DataLocation>, // Part of index ?
    pub hashes: Vec<Hash>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct RelationInfo {
    pub idx: EdgeType,
    pub forward_type: String,  // A --- HasPart---> B
    pub backward_type: String, // A <---PartOf--- B
    pub internal: bool,        // only for internal use
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Relation {
    pub from_id: Ulid,
    pub to_id: Ulid,
    pub relation_type: String,
}

pub type Source = u32;
pub type Target = u32;

#[derive(Deserialize, Serialize)]
pub struct RawRelation {
    pub source: Source,
    pub target: Target,
    pub edge_type: EdgeType,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct ServerInfo {
    pub node_id: Ulid,
    pub node_serial: u32,
    pub url: String,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct ServerState {
    pub node_id: Ulid,
    pub status: String,
}

pub struct PubKey {
    pub key_serial: u32,
    pub node_id: Ulid,
    pub key: String,
    pub decoding_key: DecodingKey,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Hash {
    pub algorithm: HashAlgorithm,
    pub value: String,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct Endpoint {
    pub id: Ulid,
    pub name: String,
    /// TODO: Add more fields
    pub description: String,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub enum SyncingStatus {
    Pending,
    Running,
    Finished,
    Error,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct DataLocation {
    pub endpoint_id: String,
    pub status: SyncingStatus,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub enum ResourceStatus {
    Initializing, // Resource initialized but no data provided
    Validating,   // Validating the resource
    Available,
    Frozen,
    Unavailable,
    Error,
    Deleted,
}

pub enum ResourceEndpointStatus {
    Pending,
    Running,
    Finished,
    Error,
}

pub enum ResourceEndpointVariant {
    Dataproxy,
    Compute,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub enum HashAlgorithm {
    Sha256,
    MD5,
}

impl Display for HashAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            HashAlgorithm::Sha256 => "Sha256",
            HashAlgorithm::MD5 => "MD5",
        };
        write!(f, "{}", name)
    }
}

#[repr(u32)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub enum Permission {
    None = 2,
    Read = 3,
    Append = 4,
    Write = 5,
    Admin = 6,
}

impl TryFrom<u32> for Permission {
    type Error = ArunaError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Ok(match value {
            PERMISSION_NONE => Permission::None,
            PERMISSION_READ => Permission::Read,
            PERMISSION_APPEND => Permission::Append,
            PERMISSION_WRITE => Permission::Write,
            PERMISSION_ADMIN => Permission::Admin,
            _ => {
                return Err(ArunaError::ConversionError {
                    from: format!("{}u32", value),
                    to: "models::Permission".to_string(),
                })
            }
        })
    }
}

// Write requests

fn default_license_tag() -> String {
    "CC-BY-SA-4.0".to_string()
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateResourceRequest {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub variant: ResourceVariant,
    #[serde(default)]
    pub labels: Vec<KeyValue>,
    #[serde(default)]
    pub identifiers: Vec<String>,
    #[serde(default)]
    pub visibility: VisibilityClass,
    #[serde(default)]
    pub authors: Vec<Author>,
    #[serde(default = "default_license_tag")]
    pub license_tag: String,
    pub parent_id: Ulid,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateRealmRequest {
    pub tag: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateGroupRequest {
    pub name: String,
    pub description: String,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateGroupResponse {
    pub group: Group,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub labels: Vec<KeyValue>,
    #[serde(default)]
    pub identifiers: Vec<String>,
    #[serde(default)]
    pub visibility: VisibilityClass,
    #[serde(default)]
    pub authors: Vec<Author>,
    #[serde(default)]
    pub license_tag: String,
    pub group_id: Ulid,
    pub realm_id: Ulid,
}

// Read requests

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema, IntoParams,
)]
pub struct GetResourceRequest {
    pub id: Ulid,
}

// Read responses
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct GetResourceResponse {
    pub resource: Resource,
    pub relations: Vec<Relation>,
}

// Write responses
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateProjectResponse {
    pub resource: Resource,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateResourceResponse {
    pub resource: Resource,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateRealmResponse {
    pub realm: Realm,
    pub admin_group_id: Ulid,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct AddGroupRequest {
    pub realm_id: Ulid,
    pub group_id: Ulid,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema)]
pub struct AddGroupResponse {}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema, IntoParams,
)]
pub struct GetRealmRequest {
    pub id: Ulid,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema, IntoParams,
)]
pub struct GetRealmResponse {
    pub realm: Realm,
    pub groups: Vec<Ulid>,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema, IntoParams,
)]
pub struct GetGroupRequest {
    pub id: Ulid,
}

#[derive(
    Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, ToSchema, IntoParams,
)]
pub struct GetGroupResponse {
    pub group: Group,
    pub members: Vec<Ulid>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IssuerType {
    ARUNA,
    OIDC,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Issuer {
    pub issuer_name: String,
    pub pubkey_endpoint: Option<String>,
    pub audiences: Vec<String>,
    pub issuer_type: IssuerType,
}

impl Issuer {
    pub async fn new_with_endpoint(
        issuer_name: String,
        pubkey_endpoint: String,
        audiences: Vec<String>,
    ) -> Self {
        Self {
            issuer_name,
            pubkey_endpoint: Some(pubkey_endpoint),
            audiences,
            issuer_type: IssuerType::OIDC,
        }
    }

    pub async fn new_with_keys(
        issuer_name: String,
        audiences: Vec<String>,
        issuer_type: IssuerType,
    ) -> Self {
        Self {
            issuer_name,
            pubkey_endpoint: None,
            audiences,
            issuer_type,
        }
    }

    pub async fn fetch_jwks(
        endpoint: &str,
    ) -> Result<(Vec<(String, DecodingKey)>, NaiveDateTime), ArunaError> {
        let client = reqwest::Client::new();
        let res = client.get(endpoint).send().await.map_err(|e| {
            tracing::error!(?e, "Error fetching JWK from endpoint");
            ArunaError::Unauthorized
        })?;
        let jwks: jsonwebtoken::jwk::JwkSet = res.json().await.map_err(|e| {
            tracing::error!(?e, "Error serializing JWK from endpoint");
            ArunaError::Unauthorized
        })?;

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
}

/// This contains claims for ArunaTokens
/// containing 3 mandatory and 2 optional fields.
///
/// - iss: Token issuer
/// - sub: User_ID or subject
/// - exp: When this token expires (by default very large number)
/// - tid: UUID from the specific token
#[derive(Debug, Serialize, Deserialize)]
pub struct ArunaTokenClaims {
    pub iss: String, // 'aruna' or oidc issuer
    pub sub: String, // Token id / OIDC Subject
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<Audience>, // Audience;
    pub exp: u64,    // Expiration timestamp
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(untagged)]
pub enum Audience {
    String(String),
    Vec(Vec<String>),
}
