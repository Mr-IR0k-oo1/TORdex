use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use s3::{Auth, Client, Credentials, Region};
use std::sync::Arc;
use tracing::info;

use tordex_types::{ArtifactStore, StoreError};

#[derive(Clone)]
pub struct MinioArtifactStore {
    client: Arc<Client>,
    bucket: String,
}

impl MinioArtifactStore {
    pub async fn connect(
        endpoint: &str,
        region: &str,
        access_key: &str,
        secret_key: &str,
        bucket: &str,
    ) -> Result<Self, StoreError> {
        let client = Client::builder(endpoint)
            .map_err(|e| StoreError::Storage(format!("invalid endpoint: {e}")))?
            .region(
                Region::new(region)
                    .map_err(|e| StoreError::Storage(format!("invalid region: {e}")))?
                    .as_str(),
            )
            .auth(Auth::Static(
                Credentials::new(access_key, secret_key)
                    .map_err(|e| StoreError::Storage(format!("invalid credentials: {e}")))?,
            ))
            .build()
            .map_err(|e| StoreError::Storage(format!("failed to build client: {e}")))?;

        info!(%bucket, "MinIO artifact store configured");

        Ok(Self {
            client: Arc::new(client),
            bucket: bucket.to_string(),
        })
    }
}

#[async_trait]
impl ArtifactStore for MinioArtifactStore {
    async fn put(
        &self,
        key: &str,
        data: &[u8],
        content_type: Option<&str>,
    ) -> Result<String, StoreError> {
        let mut req = self.client.objects().put(&self.bucket, key);
        req = req.body_bytes(Bytes::copy_from_slice(data));
        if let Some(ct) = content_type {
            req = req
                .content_type(ct)
                .map_err(|e| StoreError::Storage(format!("invalid content type: {e}")))?;
        }
        req.send()
            .await
            .map_err(|e| StoreError::Storage(format!("failed to put object: {e}")))?;
        Ok(key.to_string())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StoreError> {
        let resp = self
            .client
            .objects()
            .get(&self.bucket, key)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("Not Found") {
                    StoreError::NotFound(key.to_string())
                } else {
                    StoreError::Storage(format!("failed to get object: {e}"))
                }
            })?;
        let mut data = Vec::new();
        let mut body = resp.body;
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|e| {
                StoreError::Storage(format!("failed to read object chunk: {e}"))
            })?;
            data.extend_from_slice(&chunk);
        }
        Ok(data)
    }
}
