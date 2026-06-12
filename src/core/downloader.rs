use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use sha1::{Digest as _, Sha1};
use sha2::{Digest as _, Sha512};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// A utility for downloading files from remote URLs with streaming and hash verification.
pub struct Downloader {
    client: Client,
}

impl Downloader {
    /// Creates a new instance of the Downloader.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Downloads a file from a URL to a local path, streaming chunks and verifying its hash on the fly.
    pub async fn download_file<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        expected_hash: Option<&str>,
    ) -> Result<()> {
        let response = self.client.get(url).send().await?.error_for_status()?;
        let mut stream = response.bytes_stream();
        let mut file = File::create(&output_path).await?;

        let is_sha1 = expected_hash.map_or(false, |h| h.len() == 40);
        let mut sha1_hasher = Sha1::new();
        let mut sha512_hasher = Sha512::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;

            if expected_hash.is_some() {
                if is_sha1 {
                    sha1_hasher.update(&chunk);
                } else {
                    sha512_hasher.update(&chunk);
                }
            }
        }

        file.flush().await?;

        if let Some(expected) = expected_hash {
            let actual_hex = if is_sha1 {
                hex::encode(sha1_hasher.finalize())
            } else {
                hex::encode(sha512_hasher.finalize())
            };

            if actual_hex != expected {
                drop(file);
                let _ = tokio::fs::remove_file(&output_path).await;
                anyhow::bail!("Hash mismatch! Expected: {}, got: {}", expected, actual_hex);
            }
        }

        Ok(())
    }
}
