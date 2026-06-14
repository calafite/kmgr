use anyhow::Result;
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[derive(Clone)]
pub struct Downloader {
    client: Client,
    mp: MultiProgress,
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(crate::core::utils::HTTP_TIMEOUT_SECS))
                .build()
                .unwrap_or_else(|_| Client::new()),
            mp: MultiProgress::new(),
        }
    }

    pub fn println(&self, msg: &str) {
        let _ = self.mp.println(msg);
    }

    pub async fn download_file<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        expected_hash: Option<&str>,
    ) -> Result<()> {
        let mut attempts = 0;
        let max_attempts = 3;

        let response = loop {
            let resp = self.client.get(url).send().await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if attempts >= max_attempts {
                    anyhow::bail!("Download rate limit exceeded.");
                }
                tokio::time::sleep(Duration::from_secs(2_u64.pow(attempts))).await;
                attempts += 1;
                continue;
            }

            break resp.error_for_status()?;
        };

        let output_path = output_path.as_ref();
        let part_path = {
            let mut s = output_path.as_os_str().to_os_string();
            s.push(".part");
            std::path::PathBuf::from(s)
        };

        let total_size = response.content_length().unwrap_or(0);

        let pb = if total_size > 0 {
            let pb = self.mp.add(ProgressBar::new(total_size));
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            let pb = self.mp.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template(
                        "{spinner:.green} [{elapsed_precise}] {bytes} downloaded ({bytes_per_sec})",
                    )
                    .unwrap(),
            );
            Some(pb)
        };

        let mut stream = response.bytes_stream();
        let mut file = File::create(&part_path).await?;

        let is_sha1 = expected_hash.map_or(false, |h| h.len() == 40);
        let mut sha1_hasher = (expected_hash.is_some() && is_sha1).then(Sha1::new);
        let mut sha512_hasher = (expected_hash.is_some() && !is_sha1).then(Sha512::new);

        let mut success = false;

        let transfer_result: Result<()> = async {
            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result?;
                file.write_all(&chunk).await?;

                if let Some(ref pb) = pb {
                    pb.inc(chunk.len() as u64);
                }

                if let Some(ref mut hasher) = sha1_hasher {
                    hasher.update(&chunk);
                } else if let Some(ref mut hasher) = sha512_hasher {
                    hasher.update(&chunk);
                }
            }
            file.flush().await?;
            success = true;
            Ok(())
        }
        .await;

        if !success {
            drop(file);
            let _ = tokio::fs::remove_file(&part_path).await;
            return transfer_result;
        }

        if let Some(ref pb) = pb {
            pb.finish_and_clear();
        }

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
                let _ = tokio::fs::remove_file(&part_path).await;
                anyhow::bail!("Hash mismatch! Expected: {}, got: {}", expected, actual_hex);
            }
        }

        // Download and verification succeeded, atomically rename to final path
        drop(file);
        tokio::fs::rename(&part_path, output_path).await?;

        Ok(())
    }
}
