use crate::data_backends::storage_backend::StorageBackend;
use crate::structs::{ObjectLocation, PartETag};
use crate::trace_err;
use anyhow::{anyhow, Result};
use aruna_file::transformer::{Sink, Transformer};
use async_channel::{Receiver, Sender};
use bytes::{BufMut, BytesMut};
use std::sync::Arc;
use tracing::{debug, info_span, trace, Instrument};

pub struct BufferedS3Sink {
    backend: Arc<Box<dyn StorageBackend>>,
    buffer: BytesMut,
    target_location: ObjectLocation,
    upload_id: Option<String>,
    part_number: Option<i32>,
    single_part_upload: bool,
    tags: Vec<PartETag>,
    sum: usize,
    sender: Option<Sender<String>>,
}

impl Sink for BufferedS3Sink {}

impl BufferedS3Sink {
    #[tracing::instrument(
        level = "trace",
        skip(
            backend,
            target_location,
            upload_id,
            part_number,
            single_part_upload,
            tags,
            with_sender
        )
    )]
    pub fn new(
        backend: Arc<Box<dyn StorageBackend>>,
        target_location: ObjectLocation,
        upload_id: Option<String>,
        part_number: Option<i32>,
        single_part_upload: bool,
        tags: Option<Vec<PartETag>>,
        with_sender: bool,
    ) -> (Self, Option<Receiver<String>>) {
        let t = tags.unwrap_or_else(|| Vec::new());

        let (sx, tx) = if with_sender {
            let (tx, sx) = async_channel::bounded(2);
            (Some(sx), Some(tx))
        } else {
            (None, None)
        };

        (
            Self {
                backend,
                buffer: BytesMut::with_capacity(10_000_000),
                target_location,
                upload_id,
                part_number,
                single_part_upload,
                tags: t,
                sum: 0,
                sender: tx,
            },
            sx,
        )
    }
}

impl BufferedS3Sink {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn initialize_multipart(&mut self) -> Result<()> {
        trace!("Initializing multipart");
        self.part_number = Some(1);

        self.upload_id = Some(
            self.backend
                .init_multipart_upload(self.target_location.clone())
                .await?,
        );
        debug!("Initialized multipart");
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn upload_single(&mut self) -> Result<()> {
        trace!("Single upload");
        let backend_clone = self.backend.clone();
        let expected_len: i64 = self.buffer.len() as i64;
        let location_clone = self.target_location.clone();

        let (sender, receiver) = async_channel::bounded(10);

        trace_err!(sender.send(Ok(self.buffer.split().freeze())).await)?;

        tokio::spawn(
            async move {
                backend_clone
                    .put_object(receiver, location_clone, expected_len)
                    .await
            }
            .instrument(info_span!("upload_single_spawn")),
        )
        .await??;
        debug!(?self.target_location, "uploaded single");
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn upload_part(&mut self) -> Result<()> {
        trace!("uploading part");
        let backend_clone = self.backend.clone();
        let expected_len: i64 = self.buffer.len() as i64;
        let location_clone = self.target_location.clone();
        let pnumber = trace_err!(self
            .part_number
            .ok_or_else(|| anyhow!("PartNumber expected")))?;

        let up_id = trace_err!(self
            .upload_id
            .clone()
            .ok_or_else(|| anyhow!("Upload ID not found")))?;

        let (sender, receiver) = async_channel::bounded(10);
        trace_err!(sender.try_send(Ok(self.buffer.split().freeze())))?;

        let tag = tokio::spawn(
            async move {
                backend_clone
                    .upload_multi_object(receiver, location_clone, up_id, expected_len, pnumber)
                    .await
            }
            .instrument(info_span!("upload_part_spawn")),
        )
        .await??;
        if let Some(s) = &self.sender {
            trace_err!(s.send(tag.etag.to_string()).await)?;
        }
        self.tags.push(tag);
        self.part_number = Some(pnumber + 1);
        debug!(self.upload_id, pnumber, expected_len, "uploaded part");
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn finish_multipart(&mut self) -> Result<()> {
        trace!("Finishing multipart");
        let up_id = trace_err!(self
            .upload_id
            .clone()
            .ok_or_else(|| anyhow!("Upload ID not found")))?;
        trace_err!(
            self.backend
                .finish_multipart_upload(
                    self.target_location.clone(),
                    self.tags.clone(),
                    up_id.clone()
                )
                .await
        )?;
        debug!(up_id, "finished multipart");
        Ok(())
    }
    #[tracing::instrument(level = "trace", skip(self))]
    async fn _get_parts(&self) -> Vec<PartETag> {
        debug!(?self.tags, "get_parts");
        self.tags.clone()
    }
}

#[async_trait::async_trait]
impl Transformer for BufferedS3Sink {
    #[tracing::instrument(level = "trace", skip(self, buf, finished))]
    async fn process_bytes(&mut self, buf: &mut BytesMut, finished: bool, _: bool) -> Result<bool> {
        self.sum += buf.len();
        let len = buf.len();

        self.buffer.put(buf.split());

        if self.single_part_upload {
            if len == 0 && finished {
                self.upload_part().await?;
                return Ok(true);
            }
            Ok(false)
        } else {
            if self.buffer.len() > 5242880 {
                trace!("exceeds 5 Mib -> upload multi part");
                // 5 Mib -> initialize multipart
                if self.upload_id.is_none() {
                    self.initialize_multipart().await?;
                }
                self.upload_part().await?;
            }

            if len == 0 && finished {
                if self.upload_id.is_none() {
                    self.upload_single().await?;
                } else {
                    // Upload den Rest +
                    self.upload_part().await?;
                    if !self.single_part_upload {
                        trace!("finishing multipart");
                        self.finish_multipart().await?;
                    }
                }
                return Ok(true);
            }
            Ok(false)
        }
    }
}
