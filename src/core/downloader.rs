use anyhow::Result;
use reqwest::Client;
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub struct Downloader {
    client: Client,
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn download_file<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        expected_hash: Option<&str>,
    ) -> Result<()> {
        let response = self.client.get(url).send().await?.error_for_status()?;
        let bytes = response.bytes().await?;

        if let Some(expected) = expected_hash {
            let is_sha1 = expected.len() == 40;
            let actual_hex = if is_sha1 {
                let mut hasher = Sha1::new();
                hasher.update(&bytes);
                hex::encode(hasher.finalize())
            } else {
                let mut hasher = Sha512::new();
                hasher.update(&bytes);
                hex::encode(hasher.finalize())
            };

            if actual_hex != expected {
                anyhow::bail!("Hash mismatch! Expected: {}, got: {}", expected, actual_hex);
            }
        }

        let mut file = File::create(output_path).await?;
        file.write_all(&bytes).await?;
        Ok(())
    }
}
