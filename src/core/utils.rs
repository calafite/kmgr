use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Default timeout in seconds for all external HTTP requests.
pub const HTTP_TIMEOUT_SECS: u64 = 15;

pub async fn compute_file_hash<P: AsRef<Path>>(path: P, is_sha1: bool) -> anyhow::Result<String> {
    let path_buf = path.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        let mut file = File::open(&path_buf)?;
        let mut buffer = [0u8; 65536]; // 64KB buffer

        if is_sha1 {
            let mut hasher = Sha1::new();
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            Ok(hex::encode(hasher.finalize()))
        } else {
            let mut hasher = Sha512::new();
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            Ok(hex::encode(hasher.finalize()))
        }
    })
    .await?
}
