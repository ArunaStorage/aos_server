use std::str::FromStr;
use std::sync::Arc;

use aruna_rust_api::api::storage::models::v2::{generic_resource, Object};
use aruna_rust_api::api::storage::services::v2::object_service_server::ObjectService;
use aruna_rust_api::api::storage::services::v2::{
    CloneObjectRequest, CloneObjectResponse, CreateObjectRequest, CreateObjectResponse,
    DeleteObjectRequest, DeleteObjectResponse, FinishObjectStagingRequest,
    FinishObjectStagingResponse, GetDownloadUrlRequest, GetDownloadUrlResponse, GetObjectRequest,
    GetObjectResponse, GetObjectsRequest, GetObjectsResponse, GetUploadUrlRequest,
    GetUploadUrlResponse, SetObjectHashesRequest, SetObjectHashesResponse,
    UpdateObjectAuthorsRequest, UpdateObjectAuthorsResponse, UpdateObjectRequest,
    UpdateObjectResponse, UpdateObjectTitleRequest, UpdateObjectTitleResponse,
};
use diesel_ulid::DieselUlid;
use itertools::Itertools;
use tonic::{Request, Response, Result, Status};

use crate::auth::permission_handler::{PermissionCheck, PermissionHandler};
use crate::auth::structs::Context;
use crate::caching::cache::Cache;
use crate::caching::structs::ObjectWrapper;
use crate::database::dsls::object_dsl::ObjectWithRelations;
use crate::database::enums::DbPermissionLevel;
use crate::middlelayer::clone_request_types::CloneObject;
use crate::middlelayer::create_request_types::CreateRequest;
use crate::middlelayer::db_handler::DatabaseHandler;
use crate::middlelayer::delete_request_types::DeleteRequest;
use crate::middlelayer::finish_request_types::FinishRequest;
use crate::middlelayer::presigned_url_handler::{PresignedDownload, PresignedUpload};
use crate::middlelayer::update_request_types::{
    SetHashes, UpdateAuthor, UpdateObject, UpdateTitle,
};
use crate::search::meilisearch_client::{MeilisearchClient, ObjectDocument};
use crate::utils::grpc_utils::get_token_from_md;
use crate::utils::grpc_utils::{get_id_and_ctx, IntoGenericInner};
use crate::utils::search_utils;

crate::impl_grpc_server!(ObjectServiceImpl, search_client: Arc<MeilisearchClient>);

#[tonic::async_trait]
impl ObjectService for ObjectServiceImpl {
    async fn create_object(
        &self,
        request: Request<CreateObjectRequest>,
    ) -> Result<Response<CreateObjectResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error"
        );

        let request = CreateRequest::Object(request.into_inner());
        let mut ctxs = request.get_relation_contexts()?;
        let parent_ctx = tonic_invalid!(
            request
                .get_parent()
                .ok_or(Status::invalid_argument("Parent missing."))?
                .get_context(),
            "invalid parent"
        );
        ctxs.push(parent_ctx);
        let PermissionCheck {
            user_id, is_proxy, ..
        } = tonic_auth!(
            self.authorizer
                .check_permissions_verbose(&token, ctxs)
                .await,
            "Unauthorized"
        );
        let is_service_account = self
            .cache
            .get_user(&user_id)
            .ok_or_else(|| Status::not_found("User not found"))?
            .attributes
            .0
            .service_account;
        if is_service_account && (request.get_data_class() != 4) {
            return Err(Status::invalid_argument(
                "Workspaces have to be claimed for dataclass changes",
            ));
        }
        let (object_plus, _) = tonic_internal!(
            self.database_handler
                .create_resource(request, user_id, is_proxy)
                .await,
            "Internal database error"
        );

        self.cache.add_object(object_plus.clone());

        // Add or update object in search index
        search_utils::update_search_index(
            &self.search_client,
            &self.cache,
            vec![ObjectDocument::from(object_plus.object.clone())],
        )
        .await;

        let generic_object: generic_resource::Resource = ObjectWrapper {
            object_with_relations: object_plus.clone(),
            rules: self
                .cache
                .get_rule_bindings(&object_plus.object.id)
                .unwrap_or_default(),
        }
        .into();

        let response = CreateObjectResponse {
            object: Some(generic_object.into_inner()?),
        };

        return_with_log!(response);
    }

    async fn get_upload_url(
        &self,
        request: Request<GetUploadUrlRequest>,
    ) -> Result<Response<GetUploadUrlResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error"
        );

        let request = PresignedUpload(request.into_inner());

        let object_id = tonic_invalid!(request.get_id(), "Invalid id");
        let PermissionCheck { user_id, token, .. } = tonic_auth!(
            self.authorizer
                .check_permissions_verbose(
                    &token,
                    vec![Context::res_ctx(object_id, DbPermissionLevel::WRITE, true)]
                )
                .await,
            "Unauthorized"
        );

        let (url, upload_id) = tonic_internal!(
            self.database_handler
                .get_presigend_upload(
                    self.cache.clone(),
                    request,
                    self.authorizer.clone(),
                    user_id,
                    token,
                )
                .await,
            "Error while building presigned url"
        );

        let result = GetUploadUrlResponse {
            url,
            upload_id: upload_id.unwrap_or("".to_string()),
        };

        return_with_log!(result);
    }

    async fn get_download_url(
        &self,
        request: Request<GetDownloadUrlRequest>,
    ) -> Result<Response<GetDownloadUrlResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error"
        );

        let request = PresignedDownload(request.into_inner());

        let object_id = tonic_invalid!(request.get_id(), "Invalid id");
        let PermissionCheck { user_id, token, .. } = tonic_auth!(
            self.authorizer
                .check_permissions_verbose(
                    &token,
                    vec![Context::res_ctx(object_id, DbPermissionLevel::READ, true)]
                )
                .await,
            "Unauthorized"
        );

        let signed_url = tonic_internal!(
            self.database_handler
                .get_presigned_download(
                    self.cache.clone(),
                    self.authorizer.clone(),
                    request,
                    user_id,
                    token,
                )
                .await,
            "Error while building presigned url"
        );

        let result = GetDownloadUrlResponse { url: signed_url };

        return_with_log!(result);
    }

    async fn finish_object_staging(
        &self,
        request: Request<FinishObjectStagingRequest>,
    ) -> Result<Response<FinishObjectStagingResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error."
        );

        // Convert to FinishRequest
        let request = FinishRequest(request.into_inner());
        let object_id = &tonic_invalid!(request.get_object_id(), "Invalid object id");

        // Check permissions for finish
        let PermissionCheck {
            user_id,
            token: token_id,
            is_proxy,
            proxy_id: dataproxy_id,
        } = tonic_auth!(
            self.authorizer
                .check_permissions_verbose(
                    &token,
                    vec![Context::res_ctx(
                        tonic_invalid!(request.get_object_id(), "Invalid object_id"),
                        DbPermissionLevel::APPEND,
                        true
                    )]
                )
                .await,
            "Unauthorized"
        );

        // Manual user started
        if !is_proxy {
            // Send CompleteMultipartFinish to proxy
            tonic_internal!(
                self.database_handler
                    .complete_multipart_upload(
                        request,
                        user_id,
                        self.cache.clone(),
                        self.authorizer.clone(),
                        token_id
                    )
                    .await,
                "Multipart object finish failed"
            );

            // Fetch Object from updated cache and return
            let object = self
                .cache
                .get_wrapped_object(object_id)
                .ok_or_else(|| Status::not_found("Object not found"))?;
            let object: generic_resource::Resource = object.into();
            let response = FinishObjectStagingResponse {
                object: Some(object.into_inner()?),
            };
            return_with_log!(response);
        }

        let object = tonic_internal!(
            self.database_handler
                .finish_object(request, dataproxy_id)
                .await,
            "Internal database error."
        );

        self.cache.upsert_object(&object.object.id, object.clone());

        // Add or update object in search index
        search_utils::update_search_index(
            &self.search_client,
            &self.cache,
            vec![ObjectDocument::from(object.object.clone())],
        )
        .await;

        let object: generic_resource::Resource = ObjectWrapper {
            object_with_relations: object.clone(),
            rules: self
                .cache
                .get_rule_bindings(&object.object.id)
                .unwrap_or_default(),
        }
        .into();
        let response = FinishObjectStagingResponse {
            object: Some(object.into_inner()?),
        };
        return_with_log!(response);
    }

    async fn update_object(
        &self,
        request: Request<UpdateObjectRequest>,
    ) -> Result<Response<UpdateObjectResponse>> {
        log_received!(&request);
        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error."
        );
        let inner = request.into_inner();
        let req = UpdateObject(inner.clone());
        let object_id = tonic_invalid!(req.get_id(), "Invalid object id.");

        let ctx = Context::res_ctx(object_id, DbPermissionLevel::WRITE, true);

        let user_id = tonic_auth!(
            self.authorizer.check_permissions(&token, vec![ctx]).await,
            "Unauthorized"
        );

        // Check if service account changes dataclass
        let is_service_account = self
            .cache
            .get_user(&user_id)
            .ok_or_else(|| Status::not_found("User not found"))?
            .attributes
            .0
            .service_account;

        let (object, new_revision) = tonic_internal!(
            self.database_handler
                .update_grpc_object(inner, user_id, is_service_account)
                .await,
            "Internal database error."
        );

        self.cache.upsert_object(&object.object.id, object.clone());

        // Add or update object in search index
        search_utils::update_search_index(
            &self.search_client,
            &self.cache,
            vec![ObjectDocument::from(object.object.clone())],
        )
        .await;

        let object: generic_resource::Resource = ObjectWrapper {
            object_with_relations: object.clone(),
            rules: self
                .cache
                .get_rule_bindings(&object.object.id)
                .unwrap_or_default(),
        }
        .into();
        let response = UpdateObjectResponse {
            object: Some(object.into_inner()?),
            new_revision,
        };
        return_with_log!(response);
    }

    async fn clone_object(
        &self,
        request: Request<CloneObjectRequest>,
    ) -> Result<Response<CloneObjectResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error."
        );

        let request = CloneObject(request.into_inner());
        let object_id = tonic_invalid!(request.get_object_id(), "Invalid object id");
        let (parent_id, parent_mapping) = tonic_invalid!(request.get_parent(), "Invalid object id");
        let parent_ctx = Context::res_ctx(parent_id, DbPermissionLevel::APPEND, true);
        let object_ctx = Context::res_ctx(object_id, DbPermissionLevel::READ, true);
        let user_id = tonic_auth!(
            self.authorizer
                .check_permissions(&token, vec![parent_ctx, object_ctx])
                .await,
            "Unauthorized"
        );
        let new = tonic_internal!(
            self.database_handler
                .clone_object(&user_id, &object_id, parent_mapping)
                .await,
            "Internal clone object error"
        );
        self.cache.add_object(new.clone());

        // Add or update object in search index
        search_utils::update_search_index(
            &self.search_client,
            &self.cache,
            vec![ObjectDocument::from(new.object.clone())],
        )
        .await;

        let converted: generic_resource::Resource = ObjectWrapper {
            object_with_relations: new.clone(),
            rules: self
                .cache
                .get_rule_bindings(&new.object.id)
                .unwrap_or_default(),
        }
        .into();
        let response = CloneObjectResponse {
            object: Some(converted.into_inner()?),
        };
        return_with_log!(response);
    }

    async fn delete_object(
        &self,
        request: Request<DeleteObjectRequest>,
    ) -> Result<Response<DeleteObjectResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error."
        );

        let request = DeleteRequest::Object(request.into_inner());
        let id = tonic_invalid!(request.get_id(), "Invalid object id");

        let ctx = Context::res_ctx(id, DbPermissionLevel::ADMIN, true);

        tonic_auth!(
            self.authorizer.check_permissions(&token, vec![ctx]).await,
            "Unauthorized."
        );

        let updates: Vec<ObjectWithRelations> = tonic_internal!(
            self.database_handler.delete_resource(request).await,
            "Internal database error"
        );

        // Remove deleted resources from search index
        search_utils::remove_from_search_index(
            &self.search_client,
            updates.iter().map(|o| o.object.id).collect_vec(),
        )
        .await;

        let response = DeleteObjectResponse {};

        return_with_log!(response);
    }

    async fn get_object(
        &self,
        request: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error"
        );

        let request = request.into_inner();

        let object_id = tonic_invalid!(
            DieselUlid::from_str(&request.object_id),
            "ULID conversion error"
        );

        let ctx = Context::res_ctx(object_id, DbPermissionLevel::READ, true);

        tonic_auth!(
            self.authorizer.check_permissions(&token, vec![ctx]).await,
            "Unauthorized"
        );

        let res = self
            .cache
            .get_wrapped_object(&object_id)
            .ok_or_else(|| Status::not_found("Object not found"))?;

        let generic_object: generic_resource::Resource = res.into();

        let response = GetObjectResponse {
            object: Some(generic_object.into_inner()?),
        };

        return_with_log!(response);
    }

    async fn get_objects(
        &self,
        request: Request<GetObjectsRequest>,
    ) -> Result<Response<GetObjectsResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error"
        );

        let request = request.into_inner();

        let (ids, ctxs): (Vec<DieselUlid>, Vec<Context>) = get_id_and_ctx(request.object_ids)?;

        tonic_auth!(
            self.authorizer.check_permissions(&token, ctxs).await,
            "Unauthorized"
        );

        let res: Result<Vec<Object>> = ids
            .iter()
            .map(|id| -> Result<Object> {
                let resource: generic_resource::Resource = self
                    .cache
                    .get_wrapped_object(id)
                    .ok_or_else(|| Status::not_found("Resource not found"))?
                    .into();
                resource.into_inner()
            })
            .collect();

        let response = GetObjectsResponse { objects: res? };

        return_with_log!(response);
    }
    async fn update_object_authors(
        &self,
        request: Request<UpdateObjectAuthorsRequest>,
    ) -> Result<Response<UpdateObjectAuthorsResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error."
        );

        let request = UpdateAuthor::Object(request.into_inner());
        let object_id = tonic_invalid!(request.get_id(), "Invalid object id");
        let ctx = Context::res_ctx(object_id, DbPermissionLevel::WRITE, false);

        tonic_auth!(
            self.authorizer.check_permissions(&token, vec![ctx]).await,
            "Unauthorized"
        );

        let mut object = tonic_invalid!(
            self.database_handler.update_author(request).await,
            "Invalid update license request"
        );
        self.cache.add_stats_to_object(&mut object);

        // Add or update collection in search index
        search_utils::update_search_index(
            &self.search_client,
            &self.cache,
            vec![ObjectDocument::from(object.object.clone())],
        )
        .await;

        let rules = self.cache.get_rule_bindings(&object_id).unwrap_or_default();
        let generic_resource: generic_resource::Resource = ObjectWrapper {
            object_with_relations: object,
            rules,
        }
        .into();
        let response = UpdateObjectAuthorsResponse {
            object: Some(generic_resource.into_inner()?),
        };
        return_with_log!(response);
    }
    async fn update_object_title(
        &self,
        request: Request<UpdateObjectTitleRequest>,
    ) -> Result<Response<UpdateObjectTitleResponse>> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error."
        );

        let request = UpdateTitle::Object(request.into_inner());
        let object_id = tonic_invalid!(request.get_id(), "Invalid object id");
        let ctx = Context::res_ctx(object_id, DbPermissionLevel::WRITE, false);

        tonic_auth!(
            self.authorizer.check_permissions(&token, vec![ctx]).await,
            "Unauthorized"
        );

        let mut object = tonic_invalid!(
            self.database_handler.update_title(request).await,
            "Invalid update license request"
        );
        self.cache.add_stats_to_object(&mut object);

        // Add or update collection in search index
        search_utils::update_search_index(
            &self.search_client,
            &self.cache,
            vec![ObjectDocument::from(object.object.clone())],
        )
        .await;

        let rules = self.cache.get_rule_bindings(&object_id).unwrap_or_default();
        let generic_resource: generic_resource::Resource = ObjectWrapper {
            object_with_relations: object,
            rules,
        }
        .into();
        let response = UpdateObjectTitleResponse {
            object: Some(generic_resource.into_inner()?),
        };
        return_with_log!(response);
    }

    async fn set_object_hashes(
        &self,
        request: Request<SetObjectHashesRequest>,
    ) -> std::result::Result<Response<SetObjectHashesResponse>, Status> {
        log_received!(&request);

        let token = tonic_auth!(
            get_token_from_md(request.metadata()),
            "Token authentication error."
        );

        let request = SetHashes(request.into_inner());
        let object_id = tonic_invalid!(request.get_id(), "Invalid object id");
        let ctx = Context::res_ctx(object_id, DbPermissionLevel::WRITE, false);

        tonic_auth!(
            self.authorizer.check_permissions(&token, vec![ctx]).await,
            "Unauthorized"
        );

        let mut object = tonic_invalid!(
            self.database_handler.set_or_check_hashes(request).await,
            "Invalid update license request"
        );
        self.cache.add_stats_to_object(&mut object);

        // Add or update collection in search index
        search_utils::update_search_index(
            &self.search_client,
            &self.cache,
            vec![ObjectDocument::from(object.object.clone())],
        )
        .await;

        let rules = self.cache.get_rule_bindings(&object_id).unwrap_or_default();
        let generic_resource: generic_resource::Resource = ObjectWrapper {
            object_with_relations: object,
            rules,
        }
        .into();
        let response = SetObjectHashesResponse {
            object: Some(generic_resource.into_inner()?),
        };
        return_with_log!(response);
    }
}
