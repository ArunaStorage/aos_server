/* ----- Type ENUMs ------------------------------------------------ */
-- All ENUM types have to be created before their usage in a table
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'ObjectStatus') THEN
        CREATE TYPE "ObjectStatus" AS ENUM (
            'INITIALIZING',
            'VALIDATING',
            'AVAILABLE',
	    'UNAVAILABLE',
            'ERROR',
            'DELETED'
        );
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'DataClass') THEN
        CREATE TYPE "DataClass" AS ENUM ('PUBLIC', 'PRIVATE', 'WORKSPACE', 'CONFIDENTIAL');
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'KeyValueType') THEN
        CREATE TYPE "KeyValueType" AS ENUM ('LABEL', 'STATIC_LABEL', 'HOOK', 'STATIC_HOOK');
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'PermissionLevel') THEN
        CREATE TYPE "PermissionLevel" AS ENUM ('DENY', 'NONE', 'READ', 'APPEND', 'WRITE', 'ADMIN');
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'ObjectType') THEN
        CREATE TYPE "ObjectType" AS ENUM (
            'PROJECT',
            'COLLECTION',
            'DATASET',
            'OBJECT'
        );
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'HashType') THEN
        CREATE TYPE "HashType" AS ENUM (
            'MD5',
            'SHA256'
        );
    END IF;
END
$$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'EndpointStatus') THEN
        CREATE TYPE "EndpointStatus" AS ENUM (
            'INITIALIZING',
            'AVAILABLE',
            'DEGRADED',
            'UNAVAILABLE',
            'MAINTENANCE'
        );
    END IF;
END
$$;

/* ----- Authorization --------------------------------------------- */
-- Table with users imported from some aai
-- Join table to map users to multiple identity providers

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    display_name TEXT NOT NULL DEFAULT '',
    email VARCHAR(511) DEFAULT '',
    external_id VARCHAR(511),
    attributes JSONB NOT NULL,
    active BOOL NOT NULL DEFAULT FALSE
);

/* ----- Object Service -------------------------------------------- */
-- Table with objects which represent individual data blobs
CREATE TABLE IF NOT EXISTS objects (
    id UUID NOT NULL PRIMARY KEY, -- The unique per object id
    revision_number INT NOT NULL,
    name VARCHAR(511) NOT NULL,          -- Filename or subpath
    description VARCHAR(1023) NOT NULL,                 
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by UUID NOT NULL REFERENCES users(id),
    content_len BIGINT NOT NULL DEFAULT 0,
    count INT NOT NULL DEFAULT 0,
    key_values JSONB NOT NULL,
    object_status "ObjectStatus" NOT NULL DEFAULT 'INITIALIZING',
    data_class "DataClass" NOT NULL DEFAULT 'PRIVATE',
    object_type "ObjectType" NOT NULL DEFAULT 'PROJECT',
    external_relations JSONB NOT NULL,
    hashes JSONB NOT NULL DEFAULT '{}',
    dynamic BOOL NOT NULL DEFAULT TRUE,
    endpoints JSONB NOT NULL DEFAULT '{}',
    UNIQUE(id, object_type)
);
CREATE INDEX IF NOT EXISTS objects_pk_idx ON objects (id);

-- Table with endpoints
CREATE TABLE IF NOT EXISTS endpoints (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    host_config JSONB NOT NULL,
    is_public BOOL NOT NULL DEFAULT TRUE,
    status "EndpointStatus" NOT NULL DEFAULT 'INITIALIZING'
);

/* ----- Object Relations ------------------------------------------ */
-- Table to store custom relation types
CREATE TABLE IF NOT EXISTS relation_types (
    --id SMALLSERIAL PRIMARY KEY NOT NULL,
    relation_name VARCHAR(511) PRIMARY KEY NOT NULL 
);

-- Table to store all internal relations between objects
CREATE TABLE IF NOT EXISTS internal_relations (
    id UUID PRIMARY KEY NOT NULL,
    origin_pid UUID NOT NULL,
    origin_type "ObjectType" NOT NULL,
    relation_name VARCHAR(511) NOT NULL,
    target_pid UUID REFERENCES objects(id) ON DELETE CASCADE,
    target_type "ObjectType" NOT NULL,
    is_persistent BOOL NOT NULL DEFAULT FALSE,
    FOREIGN KEY (origin_pid, origin_type) REFERENCES objects(id, object_type) ON DELETE CASCADE,
    FOREIGN KEY (target_pid, target_type) REFERENCES objects(id, object_type) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS origin_pid_idx ON internal_relations (origin_pid);
CREATE INDEX IF NOT EXISTS target_pid_idx ON internal_relations (target_pid);

-- Table for available pubkeys
CREATE TABLE IF NOT EXISTS pub_keys (
    id SMALLSERIAL PRIMARY KEY, -- This is a serial to make jwt tokens smaller
    proxy UUID REFERENCES endpoints(id) ON DELETE CASCADE,
    pubkey TEXT NOT NULL
);

/* ----- Notification Service -------------------------------------- */
-- Table for the notification service to persist consumer
CREATE TABLE IF NOT EXISTS stream_consumers (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id),
    config JSONB NOT NULL
);
