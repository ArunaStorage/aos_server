use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::data_middleware::data_middlware::{DownloadDataMiddleware, UploadDataMiddleware};
use crate::data_middleware::empty_middleware::{EmptyMiddlewareDownload, EmptyMiddlewareUpload};
use crate::presign_handler::signer::PresignHandler;
use crate::storage_backend::storage_backend::StorageBackend;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{get, put, web, App, HttpRequest, HttpResponse, HttpServer};
use aruna_rust_api::api::storage::internal::v1::{Location, LocationType};
use async_channel::Sender;
use async_stream::stream;
use futures::{try_join, StreamExt};
use serde::Deserialize;
use tokio::fs::File;

// Size of the internally used chunks of data
pub const UPLOAD_CHUNK_SIZE: usize = 6291456;

#[derive(Deserialize, Default, Clone)]
pub struct SignedParamsQuery {
    pub signature: String,
    pub salt: String,
    pub expiry: String,
    pub upload_id: Option<String>,
    pub filename: Option<String>,
}

/// The DataServer handle the incoming and outcoming data streams
/// It uses actix-rs as a regular HTTP server
/// It is designed to consume the presigned requests generated by the gRPC API
/// All data will be handled in chunks
/// An additional middleware can be added to transform (e.g. encrypt) the incoming and outgoing data streams
pub struct DataServer {
    storage_backend: Arc<Box<dyn StorageBackend>>,
    signer: Arc<PresignHandler>,
    socket_addr: SocketAddr,
}

pub struct ServerState {
    storage_backend: Arc<Box<dyn StorageBackend>>,
    signer: Arc<PresignHandler>,
}

impl DataServer {
    pub async fn new(
        storage_backend: Arc<Box<dyn StorageBackend>>,
        signer: Arc<PresignHandler>,
        socket_addr: SocketAddr,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        return Ok(DataServer {
            storage_backend: storage_backend,
            signer: signer,
            socket_addr: socket_addr,
        });
    }

    /// Starts the DataServer to serve the actual data requests
    pub async fn serve(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let server_state = ServerState {
            storage_backend: self.storage_backend.clone(),
            signer: self.signer.clone(),
        };
        let server: Data<ServerState> = Data::new(server_state);

        HttpServer::new(move || {
            App::new()
                .service(single_upload)
                .service(multi_upload)
                .service(download)
                .app_data(server.clone())
                .wrap(Logger::default())
        })
        .bind(self.socket_addr)?
        .run()
        .await?;

        Ok(())
    }
}

/// Endpoint to download the requested object
#[get("/objects/download/{bucket}/{key:.*}")]
async fn download(
    req: HttpRequest,
    server: web::Data<ServerState>,
    path: web::Path<(String, String)>,
    sign_query: web::Query<SignedParamsQuery>,
) -> Result<HttpResponse, Error> {
    let verified = server
        .signer
        .verify_sign_url(sign_query.0.clone(), req.path().to_string())
        .unwrap();
    if !verified {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    let bucket = path.0.clone();
    let key = path.1.clone();

    let (payload_sender, data_middleware_recv) = async_channel::bounded(10);
    let (data_middleware_sender, mut object_handler_recv) = async_channel::bounded(10);

    let downloader_middleware =
        EmptyMiddlewareDownload::new(data_middleware_sender.clone(), data_middleware_recv.clone())
            .await;

    let stream = stream! {
        while let Some(value) = object_handler_recv.next().await {
            yield Ok(value);
        }
        //Type annotation of stream is ugly, therefor a pseudoerror is yielded, this should never be executed.
        if !true {
            match File::open("foo").await {
                Ok(_) => todo!(),
                Err(e) => {
                    yield Err(e);
                }
            }
        }
    };

    let location = Location {
        bucket: bucket,
        path: key,
        r#type: LocationType::Unspecified as i32,
    };

    let cloned_server = server.clone();
    tokio::spawn(async move {
        cloned_server
            .storage_backend
            .download(location, None, payload_sender.clone())
            .await?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync + 'static>>(())
    });

    tokio::spawn(async move { downloader_middleware.handle_stream().await });

    let response = HttpResponse::Ok()
        .append_header(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(String::from(
                sign_query.0.filename.unwrap_or("".to_string()),
            ))],
        })
        .streaming(stream);

    return Ok(response);
}

/// Endpoint to upload the requested object in one piece
#[put("/objects/upload/single/{bucket}/{key:.*}")]
async fn single_upload(
    req: HttpRequest,
    server: web::Data<ServerState>,
    payload: web::Payload,
    path: web::Path<(String, String)>,
    sign_query: web::Query<SignedParamsQuery>,
) -> Result<HttpResponse, Error> {
    let verified = server
        .signer
        .verify_sign_url(sign_query.0, req.path().to_string())
        .unwrap();
    if !verified {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    let content_len = match req.headers().get("Content-Length") {
        Some(value) => value,
        None => {
            return Ok(HttpResponse::BadRequest().body("could not read Content-Length header"));
        }
    };
    let content_len_string = match std::str::from_utf8(content_len.as_bytes()) {
        Ok(value) => value,
        Err(e) => {
            log::debug!("{}", e);
            return Ok(HttpResponse::BadRequest().body("could not read Content-Length header"));
        }
    };
    let content_len = match content_len_string.parse::<i64>() {
        Ok(value) => value,
        Err(e) => {
            log::debug!("{}", e);
            return Ok(HttpResponse::BadRequest().body("could not read Content-Length header"));
        }
    };

    let (payload_sender, data_middleware_recv) = async_channel::bounded(10);
    let (data_middleware_sender, object_handler_recv) = async_channel::bounded(10);

    let middleware = EmptyMiddlewareUpload::new(data_middleware_sender, data_middleware_recv).await;
    let payload_handler = handle_payload(payload, payload_sender);
    let middleware_handler = middleware.handle_stream();

    let location = Location {
        bucket: path.0.to_string(),
        path: path.1.to_string(),
        r#type: LocationType::Unspecified as i32,
    };

    let s3_handler =
        server
            .storage_backend
            .upload_object(object_handler_recv, location, content_len);

    if let Err(err) = try_join!(payload_handler, middleware_handler, s3_handler) {
        log::error!("{}", err);
        return Ok(HttpResponse::InternalServerError().finish());
    }

    return Ok(HttpResponse::Ok().finish());
}

/// Endpoint to upload the requested object in one piece
#[put("/objects/upload/multi/{part}/{bucket}/{key:.*}")]
async fn multi_upload(
    req: HttpRequest,
    server: web::Data<ServerState>,
    payload: web::Payload,
    path: web::Path<(i32, String, String)>,
    sign_query: web::Query<SignedParamsQuery>,
) -> Result<HttpResponse, Error> {
    let verified = server
        .signer
        .verify_sign_url(sign_query.0.clone(), req.path().to_string())
        .unwrap();
    if !verified {
        return Ok(HttpResponse::Unauthorized().finish());
    }

    let content_len = match req.headers().get("Content-Length") {
        Some(value) => value,
        None => {
            return Ok(HttpResponse::BadRequest().body("could not read Content-Length header"));
        }
    };
    let content_len_string = match std::str::from_utf8(content_len.as_bytes()) {
        Ok(value) => value,
        Err(e) => {
            log::debug!("{}", e);
            return Ok(HttpResponse::BadRequest().body("could not read Content-Length header"));
        }
    };
    let content_len = match content_len_string.parse::<i64>() {
        Ok(value) => value,
        Err(e) => {
            log::debug!("{}", e);
            return Ok(HttpResponse::BadRequest().body("could not read Content-Length header"));
        }
    };

    let upload_id = match sign_query.0.upload_id.clone() {
        Some(value) => value,
        None => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "upload id required in multipart upload",
            ));
        }
    };

    let (payload_sender, data_middleware_recv) = async_channel::bounded(10);
    let (data_middleware_sender, object_handler_recv) = async_channel::bounded(10);

    let middleware = EmptyMiddlewareUpload::new(data_middleware_sender, data_middleware_recv).await;
    let payload_handler = handle_payload(payload, payload_sender);
    let middleware_handler = middleware.handle_stream();

    let location = Location {
        bucket: path.0.to_string(),
        path: path.1.to_string(),
        r#type: LocationType::Unspecified as i32,
    };

    let s3_handler = server.storage_backend.upload_multi_object(
        object_handler_recv,
        location,
        upload_id,
        content_len,
        path.0,
    );

    let (_, _, etag) = match try_join!(payload_handler, middleware_handler, s3_handler) {
        Ok(values) => values,
        Err(err) => {
            log::error!("{}", err);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let response = HttpResponse::Ok().append_header(("ETag", etag)).finish();
    return Ok(response);
}

async fn handle_payload(
    mut payload: web::Payload,
    sender: Sender<bytes::Bytes>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let mut count = 0;
    let mut count_2 = 0;
    let mut bytes = web::BytesMut::new();
    while let Some(item) = payload.next().await {
        count = count + 1;
        let item = item.unwrap();
        bytes.extend_from_slice(&item);
        if bytes.len() > UPLOAD_CHUNK_SIZE {
            count_2 = count_2 + 1;
            let bytes_for_send = bytes::Bytes::from(bytes);

            sender.send(bytes_for_send).await?;
            bytes = web::BytesMut::new();
        }
    }

    let bytes_for_send = bytes::Bytes::from(bytes);
    sender.send(bytes_for_send).await?;

    Ok(())
}

mod tests {}
