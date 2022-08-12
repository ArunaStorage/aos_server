use std::env;
use std::sync::Arc;

use tonic::transport::Server;

use crate::api::aruna::api::storage::internal::v1::internal_proxy_service_client::InternalProxyServiceClient;
use crate::api::aruna::api::storage::services::v1::object_service_server::ObjectServiceServer;
use crate::server::services::authz::Authz;
use crate::{
    api::aruna::api::storage::services::v1::collection_service_server::CollectionServiceServer,
    database::connection::Database,
};

use super::services::collection::CollectionServiceImpl;
use super::services::object::ObjectServiceImpl;

pub struct ServiceServer {}

impl ServiceServer {
    pub async fn run(&self) {
        // ToDo: Implement config handling from YAML config file

        // Connects to database
        let db = Database::new();
        let db_ref = Arc::new(db);

        // Connects to data proxy
        let data_proxy_url = env::var("DATA_PROXY_URL").expect("DATA_PROXY_URL must be set");
        let data_proxy = InternalProxyServiceClient::connect(data_proxy_url.to_string())
            .await
            .unwrap(); //ToDo: Replace unwrap() with retry strategy

        // Upstart server
        let addr = "[::1]:50051".parse().unwrap();
        let authz = Arc::new(Authz::new(db_ref.clone()).await);
        let collection_service = CollectionServiceImpl::new(db_ref.clone(), authz.clone()).await;
        let object_service =
            ObjectServiceImpl::new(db_ref.clone(), authz.clone(), data_proxy.clone()).await;

        println!("ArunaServer listening on {}", addr);

        Server::builder()
            .add_service(CollectionServiceServer::new(collection_service))
            .add_service(ObjectServiceServer::new(object_service))
            .serve(addr)
            .await
            .unwrap();
    }
}
