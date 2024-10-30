use crate::models::models::RelationInfo;

pub struct Field {
    pub name: &'static str,
    pub index: u32,
}

// Milli index internal field ids
pub const FIELDS: &[Field] = &[
    Field {
        name: "id",
        index: 0,
    }, // 0 UUID - This is the primary key             | ALL
    Field {
        name: "variant",
        index: 1,
    }, // 1 Int - NodeVariant                          | ALL
    Field {
        name: "name",
        index: 2,
    }, // 2 String - Name of the resource              | ALL
    Field {
        name: "description",
        index: 3,
    }, // 3 String - Description of the resource       | ALL
    Field {
        name: "labels",
        index: 4,
    }, // 4 Value - Labels of the resource             | Resource
    Field {
        name: "identifiers",
        index: 5,
    }, // 5 Value - List of external identifiers       | Resource
    Field {
        name: "content_len",
        index: 6,
    }, // 6 Int - Length of the content                | Resource
    Field {
        name: "count",
        index: 7,
    }, // 7 Int - Count of the resource                | Resource
    Field {
        name: "visibility",
        index: 8,
    }, // 8 Int - Visibility of the resource           | Resource
    Field {
        name: "created_at",
        index: 9,
    }, // 9 Int - Creation time of the resource        | ALL
    Field {
        name: "last_modified",
        index: 10,
    }, // 10 Int - Last update time of the resource    | ALL
    Field {
        name: "authors",
        index: 11,
    }, // 11 Value - List of authors of the resource   | Resource
    Field {
        name: "locked",
        index: 12,
    }, // 12 Bool - Is the resource read_only          | Resource
    Field {
        name: "license",
        index: 13,
    }, // 13 String - License of the resource          | Resource
    Field {
        name: "hashes",
        index: 14,
    }, // 14 Value - Hashes of the resource            | Resource
    Field {
        name: "location",
        index: 15,
    }, // 15 Value - Location of the resource          | Resource
    Field {
        name: "tags",
        index: 16,
    }, // 16 Value - Tags of a realm                   | Realm
    Field {
        name: "expires_at",
        index: 17,
    }, // 17 Int - Expiration time of the resource     | Token
    Field {
        name: "first_name",
        index: 18,
    }, // 18 String - First name of the user           | User
    Field {
        name: "last_name",
        index: 19,
    }, // 19 String - Last name of the user            | User
    Field {
        name: "email",
        index: 20,
    }, // 20 String - Email of the user                | User
    Field {
        name: "global_admin",
        index: 21,
    }, // 21 Bool - Is the user a global admin         | User
    Field {
        name: "tag",
        index: 22,
    }, // 22 String - Tag or Title of a resource        | Realm / Resource
];

pub mod relation_types {
    pub const HAS_PART: u32 = 0u32;
    pub const OWNS_PROJECT: u32 = 1u32;
    pub const PERMISSION_NONE: u32 = 2u32;
    pub const PERMISSION_READ: u32 = 3u32;
    pub const PERMISSION_APPEND: u32 = 4u32;
    pub const PERMISSION_WRITE: u32 = 5u32;
    pub const PERMISSION_ADMIN: u32 = 6u32;
    pub const SHARES_PERMISSION: u32 = 7u32;
    pub const OWNED_BY_USER: u32 = 8u32;
    pub const GROUP_PART_OF_REALM: u32 = 9u32;
    pub const GROUP_ADMINISTRATES_REALM: u32 = 10u32;
    pub const REALM_USES_ENDPOINT: u32 = 11u32;
}

pub fn const_relations() -> [RelationInfo; 12] {
    [
        // Resource only
        // Target can only have one origin
        RelationInfo {
            idx: relation_types::HAS_PART,
            forward_type: "HasPart".to_string(),
            backward_type: "PartOf".to_string(),
            internal: false,
        },
        // Group -> Project only
        RelationInfo {
            idx: relation_types::OWNS_PROJECT,
            forward_type: "OwnsProject".to_string(),
            backward_type: "ProjectOwnedBy".to_string(),
            internal: false,
        },
        //  User / Group / Token / ServiceAccount -> Resource only
        RelationInfo {
            idx: relation_types::PERMISSION_NONE,
            forward_type: "PermissionNone".to_string(),
            backward_type: "PermissionNone".to_string(),
            internal: true, // -> Displayed by resource request
        },
        RelationInfo {
            idx: 3,
            forward_type: "PermissionRead".to_string(),
            backward_type: "PermissionRead".to_string(),
            internal: true,
        },
        RelationInfo {
            idx: 4,
            forward_type: "PermissionAppend".to_string(),
            backward_type: "PermissionAppend".to_string(),
            internal: true,
        },
        RelationInfo {
            idx: 5,
            forward_type: "PermissionWrite".to_string(),
            backward_type: "PermissionWrite".to_string(),
            internal: true,
        },
        RelationInfo {
            idx: 6,
            forward_type: "PermissionAdmin".to_string(),
            backward_type: "PermissionAdmin".to_string(),
            internal: true,
        },
        // Group -> Group only
        RelationInfo {
            idx: 7,
            forward_type: "SharesPermissionTo".to_string(),
            backward_type: "PermissionSharedFrom".to_string(),
            internal: true,
        },
        // Token -> User only
        RelationInfo {
            idx: 8,
            forward_type: "OwnedByUser".to_string(),
            backward_type: "UserOwnsToken".to_string(),
            internal: true,
        },
        // Group -> Realm
        RelationInfo {
            idx: 9,
            forward_type: "GroupPartOfRealm".to_string(),
            backward_type: "RealmHasGroup".to_string(),
            internal: true,
        },
        // Mutually exclusive with GroupPartOfRealm
        // Can only have a connection to one realm
        // Group -> Realm
        RelationInfo {
            idx: 10,
            forward_type: "GroupAdministratesRealm".to_string(),
            backward_type: "RealmAdministratedBy".to_string(),
            internal: true,
        },
        // Realm -> Endpoint
        RelationInfo {
            idx: 11,
            forward_type: "RealmUsesEndpoint".to_string(),
            backward_type: "EndpointUsedByRealm".to_string(),
            internal: true,
        },
    ]
}
