use anyhow::anyhow;
use anyhow::Result;
use aruna_rust_api::api::dataproxy::services::v2::bundler_service_server::BundlerServiceServer;
use aruna_rust_api::api::dataproxy::services::v2::dataproxy_replication_service_server::DataproxyReplicationServiceServer;
use aruna_rust_api::api::dataproxy::services::v2::dataproxy_user_service_server::DataproxyUserServiceServer;
use caching::cache::Cache;
use data_backends::{s3_backend::S3Backend, storage_backend::StorageBackend};
use futures_util::TryFutureExt;
use grpc_api::bundler::BundlerServiceImpl;
use grpc_api::{
    proxy_service::DataproxyReplicationServiceImpl, user_service::DataproxyUserServiceImpl,
};
use lazy_static::lazy_static;
use std::{net::SocketAddr, sync::Arc};
use tokio::try_join;
use tonic::transport::Server;
use tracing::error;
use tracing::info_span;
use tracing::trace;
use tracing::Instrument;
use tracing_subscriber::EnvFilter;

mod bundler;
mod caching;
mod data_backends;
mod database;
mod replication;
mod s3_frontend;
// mod helpers;
mod grpc_api;
mod structs;
#[macro_use]
mod macros;
mod auth;
mod config;
mod helpers;

use crate::config::Config;
use crate::data_backends::filesystem_backend::FSBackend;
use crate::replication::replication_handler::ReplicationHandler;

lazy_static! {
    static ref CONFIG: Config = {
        dotenvy::from_filename(".env").ok();
        let config_file = dotenvy::var("CONFIG").unwrap_or("config.toml".to_string());
        let mut config: Config =
            toml::from_str(std::fs::read_to_string(config_file).unwrap().as_str()).unwrap();
        config.validate().unwrap();
        config
    };
}

#[tracing::instrument(level = "trace", skip())]
#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::from_filename(".env").ok();

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or("none".into())
        .add_directive("aos_data_proxy=trace".parse()?);

    let subscriber = tracing_subscriber::fmt()
        //.with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        // Use a more compact, abbreviated log format
        .compact()
        // Set LOG_LEVEL to
        .with_env_filter(filter)
        // Display source code file paths
        .with_file(true)
        // Display source code line numbers
        .with_line_number(true)
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    trace!("init storage backend");

    let backend: Box<dyn StorageBackend> = match CONFIG.backend {
        config::Backend::S3 { .. } => {
            Box::new(S3Backend::new(CONFIG.proxy.endpoint_id.to_string()).await?)
        }
        config::Backend::FileSystem { .. } => {
            Box::new(FSBackend::new(CONFIG.proxy.endpoint_id.to_string()).await?)
        }
    };

    let storage_backend: Arc<Box<dyn StorageBackend>> = Arc::new(backend);

    trace!("init cache");
    let (sender, receiver) = async_channel::unbounded();
    let cache = Cache::new(
        CONFIG.proxy.aruna_url.clone(),
        CONFIG.persistence.is_some(),
        CONFIG.proxy.endpoint_id,
        CONFIG
            .proxy
            .clone()
            .private_key
            .ok_or_else(|| anyhow!("Private key not set"))?,
        CONFIG.proxy.serial,
        sender.clone(),
        Some(storage_backend.clone()),
    )
    .await?;

    trace!("init replication handler");
    let replication_handler = ReplicationHandler::new(
        receiver,
        storage_backend.clone(),
        CONFIG.proxy.endpoint_id.to_string(),
        cache.clone(),
    );
    tokio::spawn(async move {
        let replication = replication_handler.run().await;
        if let Err(err) = replication {
            trace!("{err}");
        };
    });

    trace!("init s3 server");
    let cache_clone = cache.clone();
    let s3_server = if let Some(frontend) = &CONFIG.frontend {
        Some(
            s3_frontend::s3server::S3Server::new(
                &frontend.server,
                frontend.hostname.to_string(),
                storage_backend.clone(),
                cache,
            )
            .await?,
        )
    } else {
        None
    };
    trace!("init grpc server");

    let proxy_grpc_addr = CONFIG.proxy.grpc_server.parse::<SocketAddr>()?;

    let grpc_server_handle = tokio::spawn(
        async move {
            let mut builder = Server::builder()
                .add_service(DataproxyReplicationServiceServer::new(
                    DataproxyReplicationServiceImpl::new(
                        cache_clone.clone(),
                        sender,
                        storage_backend,
                    ),
                ))
                .add_service(DataproxyUserServiceServer::new(
                    DataproxyUserServiceImpl::new(cache_clone.clone()),
                ));

            if let Some(frontend) = &CONFIG.frontend {
                builder = builder.add_service(BundlerServiceServer::new(BundlerServiceImpl::new(
                    cache_clone.clone(),
                    frontend.hostname.to_string(),
                    true,
                )));
            };

            builder.serve(proxy_grpc_addr).await
        }
        .instrument(info_span!("grpc_server_run")),
    )
    .map_err(|e| {
        error!(error = ?e, msg = e.to_string());
        anyhow!("an error occured {e}")
    });

    if let Some(s3_server) = s3_server {
        match try_join!(s3_server.run(), grpc_server_handle) {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("{}", err);
                Err(err)
            }
        }
    } else {
        grpc_server_handle.await??;
        Ok(())
    }
}
