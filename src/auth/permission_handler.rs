use super::{
    structs::{Context, ContextVariant},
    token_handler::{Action, TokenHandler},
};
use crate::{caching::cache::Cache, database::enums::DbPermissionLevel};
use anyhow::Result;
use diesel_ulid::DieselUlid;
use std::sync::Arc;

pub struct PermissionHandler {
    cache: Arc<Cache>,
    pub token_handler: Arc<TokenHandler>,
}

impl PermissionHandler {
    pub fn new(cache: Arc<Cache>, token_handler: Arc<TokenHandler>) -> Self {
        Self {
            cache: cache.clone(),
            token_handler, //Arc::new(TokenHandler::new(cache, realm_info.to_string())),
        }
    }

    pub async fn check_permissions_verbose(
        &self,
        token: &str,
        mut ctxs: Vec<Context>,
    ) -> Result<(DieselUlid, Option<DieselUlid>, bool), tonic::Status> {
        // What are the cases?
        // 1. User Aruna token       --> (user_id, token_id)
        // 2. User OIDC token        --> (user_id, None)
        // 3. Endpoint signed token  --> (user_id, ?)
        // 4. Endpoint notifications --> (endpoint_id, None)
        let (main_id, associated_id, permissions, is_proxy, proxy_action) = tonic_auth!(
            self.token_handler.process_token(token).await,
            "Unauthorized"
        );

        // Individual permission checking if token is signed from Dataproxy
        if is_proxy {
            // Add Dataproxy context
            ctxs.push(Context::proxy());

            if let Some(action) = proxy_action {
                if action == Action::Impersonate {
                    //Case 1: Impersonate
                    //  - Check if provided contexts are proxy/activated/resource only
                    for ctx in &ctxs {
                        dbg!(&ctx);
                        match ctx.variant {
                            ContextVariant::Activated
                            | ContextVariant::GlobalProxy
                            | ContextVariant::Resource(_) => {}
                            _ => return Err(tonic::Status::invalid_argument(
                                "Only resource functionality allowed for Dataproxy signed tokens",
                            )),
                        }
                    }
                } else if action == Action::FetchInfo {
                    //Case 2: FetchInfo
                    //  - Only get functions -> DbPermissionLevel::READ in contexts
                    for ctx in &ctxs {
                        dbg!(&ctx);
                        match ctx.variant {
                            ContextVariant::Activated | ContextVariant::GlobalProxy => {}
                            ContextVariant::Resource((_, perm))
                            | ContextVariant::User((_, perm)) => {
                                if perm > DbPermissionLevel::READ {
                                    return Err(tonic::Status::permission_denied(
                                        "Only get functions allowed",
                                    ));
                                }
                            }
                            _ => {
                                return Err(tonic::Status::permission_denied(
                                    "Only get functions allowed",
                                ))
                            }
                        }
                    }
                    //unimplemented!("Permission check for Dataproxy notification fetch not yet implemented")
                }

                if self.cache.check_proxy_ctxs(&main_id, &ctxs) {
                    return Ok((main_id, associated_id, true));
                } else {
                    return Err(tonic::Status::unauthenticated(
                        "Invalid proxy authentication",
                    ));
                }
            } else {
                return Err(tonic::Status::internal("Missing intent action"));
            }
        }

        // Check permissions for standard ArunaServer user token
        if self
            .cache
            .check_permissions_with_contexts(&ctxs, &permissions, &main_id)
        {
            Ok((main_id, associated_id, false))
        } else {
            Err(tonic::Status::unauthenticated("Invalid permissions"))
        }
    }

    ///ToDo: Rust Doc
    pub async fn check_permissions(
        &self,
        token: &str,
        ctxs: Vec<Context>,
    ) -> Result<DieselUlid, tonic::Status> {
        let (user_id, _, _) = self.check_permissions_verbose(token, ctxs).await?;
        Ok(user_id)
    }

    pub async fn check_unregistered_oidc(&self, token: &str) -> Result<String> {
        Ok(self.token_handler.process_oidc_token(token).await?.sub)
    }
}
