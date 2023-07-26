use super::update_request_types::UpdateObject;
use crate::database::crud::CrudDb;
use crate::database::dsls::object_dsl::Object;
use crate::middlelayer::db_handler::DatabaseHandler;
use crate::middlelayer::update_request_types::{
    DataClassUpdate, DescriptionUpdate, KeyValueUpdate, NameUpdate,
};
use anyhow::{anyhow, Result};
use aruna_rust_api::api::storage::models::v2::generic_resource;
use aruna_rust_api::api::storage::services::v2::UpdateObjectRequest;
use diesel_ulid::DieselUlid;
use postgres_types::Json;

impl DatabaseHandler {
    pub async fn update_dataclass(
        &self,
        request: DataClassUpdate,
    ) -> Result<(
        generic_resource::Resource,
        DieselUlid,
        aruna_cache::structs::Resource,
    )> {
        let mut client = self.database.get_client().await?;
        let transaction = client.transaction().await?;
        let transaction_client = transaction.client();
        let dataclass = request.get_dataclass()?;
        let id = request.get_id()?;
        let old_class: i32 = Object::get(id, transaction_client)
            .await?
            .ok_or(anyhow!("Resource not found."))?
            .data_class
            .into();
        if old_class > dataclass.clone().into() {
            return Err(anyhow!("Dataclasses can only be relaxed."));
        }
        Object::update_dataclass(id, dataclass, transaction_client).await?;
        let object_with_relations =
            Object::get_object_with_relations(&id, transaction_client).await?;

        transaction.commit().await?;
        let object = object_with_relations.clone().object;
        Ok((
            object_with_relations.try_into()?,
            object.get_shared(),
            object.get_cache_resource(),
        ))
    }
    pub async fn update_name(
        &self,
        request: NameUpdate,
    ) -> Result<(
        generic_resource::Resource,
        DieselUlid,
        aruna_cache::structs::Resource,
    )> {
        let mut client = self.database.get_client().await?;
        let transaction = client.transaction().await?;
        let transaction_client = transaction.client();
        let name = request.get_name();
        let id = request.get_id()?;
        Object::update_name(id, name, transaction_client).await?;
        let object_with_relations =
            Object::get_object_with_relations(&id, transaction_client).await?;

        transaction.commit().await?;
        let object = object_with_relations.clone().object;
        Ok((
            object_with_relations.try_into()?,
            object.get_shared(),
            object.get_cache_resource(),
        ))
    }
    pub async fn update_description(
        &self,
        request: DescriptionUpdate,
    ) -> Result<(
        generic_resource::Resource,
        DieselUlid,
        aruna_cache::structs::Resource,
    )> {
        let mut client = self.database.get_client().await?;
        let transaction = client.transaction().await?;
        let transaction_client = transaction.client();
        let description = request.get_description();
        let id = request.get_id()?;
        Object::update_description(id, description, transaction_client).await?;
        let object_with_relations =
            Object::get_object_with_relations(&id, transaction_client).await?;
        transaction.commit().await?;
        let object = object_with_relations.clone().object;
        Ok((
            object_with_relations.try_into()?,
            object.get_shared(),
            object.get_cache_resource(),
        ))
    }
    pub async fn update_keyvals(
        &self,
        request: KeyValueUpdate,
    ) -> Result<(
        generic_resource::Resource,
        DieselUlid,
        aruna_cache::structs::Resource,
    )> {
        let mut client = self.database.get_client().await?;
        let transaction = client.transaction().await?;
        let transaction_client = transaction.client();
        let id = request.get_id()?;
        let (add_key_values, rm_key_values) = request.get_keyvals()?;
        if !add_key_values.0.is_empty() {
            for kv in add_key_values.0 {
                Object::add_key_value(&id, transaction_client, kv).await?;
            }
        } else if !rm_key_values.0.is_empty() {
            let object = Object::get(id, transaction_client)
                .await?
                .ok_or(anyhow!("Dataset does not exist."))?;
            for kv in rm_key_values.0 {
                object.remove_key_value(transaction_client, kv).await?;
            }
        } else {
            return Err(anyhow!(
                "Both add_key_values and remove_key_values are empty.",
            ));
        }

        let object_with_relations =
            Object::get_object_with_relations(&id, transaction_client).await?;
        transaction.commit().await?;

        let object = object_with_relations.clone().object;
        Ok((
            object_with_relations.try_into()?,
            object.get_shared(),
            object.get_cache_resource(),
        ))
    }
    pub async fn update_grpc_object(
        &self,
        request: UpdateObjectRequest,
        user_id: DieselUlid,
    ) -> Result<(
        generic_resource::Resource,
        DieselUlid,
        aruna_cache::structs::Resource,
        bool, // Creates revision
    )> {
        let mut client = self.database.get_client().await?;
        let transaction = client.transaction().await?;
        let transaction_client = transaction.client();
        let req = UpdateObject(request.clone());
        let id = req.get_id()?;
        let old = Object::get(id, transaction_client)
            .await?
            .ok_or(anyhow!("Object not found."))?;
        let flag = if request.name.is_some()
            || !request.remove_key_values.is_empty()
            || !request.hashes.is_empty()
        {
            // Create new object
            let create_object = Object {
                id: DieselUlid::generate(),
                shared_id: id,
                content_len: old.content_len,
                count: 1,
                revision_number: old.revision_number + 1,
                external_relations: old.clone().external_relations,
                created_at: None,
                created_by: user_id,
                data_class: req.get_dataclass(old.clone())?,
                description: req.get_description(old.clone()),
                name: req.get_name(old.clone()),
                key_values: Json(req.get_all_kvs(old.clone())?),
                hashes: Json(req.get_hashes(old)?),
                object_type: crate::database::enums::ObjectType::OBJECT,
                object_status: crate::database::enums::ObjectStatus::AVAILABLE,
                dynamic: false,
            };
            create_object.create(transaction_client).await?;
            if let Some(p) = request.parent {
                let relation = UpdateObject::add_parent_relation(id, p)?;
                relation.create(transaction_client).await?;
            }
            true
        } else {
            // Update in place
            let update_object = Object {
                id: old.id,
                shared_id: id,
                content_len: old.content_len,
                count: 1,
                revision_number: old.revision_number,
                external_relations: old.clone().external_relations,
                created_at: None,
                created_by: old.created_by,
                data_class: req.get_dataclass(old.clone())?,
                description: req.get_description(old.clone()),
                name: old.clone().name,
                key_values: Json(req.get_add_keyvals(old.clone())?),
                hashes: old.hashes,
                object_type: crate::database::enums::ObjectType::OBJECT,
                object_status: crate::database::enums::ObjectStatus::AVAILABLE,
                dynamic: false,
            };
            update_object.update(transaction_client).await?;
            if let Some(p) = request.parent {
                let relation = UpdateObject::add_parent_relation(id, p)?;
                relation.create(transaction_client).await?;
            }
            false
        };
        let grpc_object = Object::get_object_with_relations(&id, transaction_client).await?;
        transaction.commit().await?;
        let object = grpc_object.clone().object;
        Ok((
            grpc_object.try_into()?,
            object.get_shared(),
            object.get_cache_resource(),
            flag,
        ))
    }
}
