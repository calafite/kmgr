use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use sha1::Sha1;
use sha2::{Digest, Sha512};
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
        let mut sha1_hasher = (expected_hash.is_some() && is_sha1).then(Sha1::new);
        let mut sha512_hasher = (expected_hash.is_some() && !is_sha1).then(Sha512::new);

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;

            if let Some(ref mut hasher) = sha1_hasher {
                hasher.update(&chunk);
            } else if let Some(ref mut hasher) = sha512_hasher {
                hasher.update(&chunk);
            }
        }

        file.flush().await?;

        if let Some(expected) = expected_hash {
            let actual_hex = if is_sha1 {
                hex::encode(
                    sha1_hasher
                        .expect("SHA1 hasher should be initialized")
                        .finalize(),
                )
            } else {
                hex::encode(
                    sha512_hasher
                        .expect("SHA512 hasher should be initialized")
                        .finalize(),
                )
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
