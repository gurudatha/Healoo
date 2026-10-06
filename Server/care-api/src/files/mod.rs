//! Attachment storage (design doc 7.4). Files never go through Cassandra; items store
//! `obj:<key>` URIs and the API hands out short-lived URLs only after the policy check.
//!
//! * [`DiskStore`] — files under FILES_DIR, served by this API at `/files/<key>` with
//!   HMAC-signed, expiring URLs. Ideal for a LAN trial: no storage service, no extra ports.
//! * [`S3Store`] — any S3-compatible store (SeaweedFS in the internet compose file, or AWS S3) with
//!   presigned URLs. For internet deployments; also works on the LAN.

use crate::config::{Config, StoreKind};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{path::PathBuf, sync::Arc, time::Duration};
use url::Url;

pub const URI_PREFIX: &str = "obj:";
/// How long download and upload links stay valid (doc 7.4): uploads.download_link_minutes in server.json.
pub fn url_ttl() -> Duration { crate::settings::settings().uploads.link_ttl() }

pub fn object_uri(key: &str) -> String { format!("{URI_PREFIX}{key}") }
pub fn key_of(uri: &str) -> Option<&str> { uri.strip_prefix(URI_PREFIX) }

#[async_trait]
pub trait ObjectStore: Send + Sync + 'static {
    fn kind(&self) -> StoreKind;
    async fn presign_put(&self, key: &str, mime: &str) -> Result<String>;
    async fn presign_get(&self, key: &str) -> Result<String>;
    async fn get(&self, key: &str) -> Result<Bytes>;
    async fn put(&self, key: &str, data: Bytes, mime: &str) -> Result<()>;
}

/// Returns the store, plus the concrete disk store when the API itself must serve `/files`.
pub async fn build(cfg: &Config, public_base: &Url) -> Result<(Arc<dyn ObjectStore>, Option<Arc<DiskStore>>)> {
    Ok(match cfg.store {
        StoreKind::Disk => {
            let disk = Arc::new(DiskStore::new(cfg.files_dir.clone(), &cfg.files_secret, public_base.clone())?);
            (disk.clone() as Arc<dyn ObjectStore>, Some(disk))
        }
        StoreKind::S3 => (Arc::new(S3Store::new(cfg).await?) as Arc<dyn ObjectStore>, None),
    })
}

/// Keys look like `uploads/<user>/<ulid>-<name>`; reject anything that could escape FILES_DIR.
pub fn safe_key(key: &str) -> Result<&str> {
    if key.is_empty() || key.starts_with('/') || key.split('/').any(|p| p == ".." || p.is_empty()) || key.contains('\\') {
        bail!("invalid object key");
    }
    Ok(key)
}

// ---------------- Disk (LAN) ----------------

pub struct DiskStore {
    root: PathBuf,
    secret: Vec<u8>,
    base: Url,
}

impl DiskStore {
    pub fn new(root: PathBuf, secret: &str, base: Url) -> Result<Self> {
        std::fs::create_dir_all(&root).with_context(|| format!("creating {}", root.display()))?;
        Ok(Self { root, secret: secret.as_bytes().to_vec(), base })
    }

    fn sign(&self, method: &str, key: &str, exp: i64, mime: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret).expect("any key length");
        mac.update(format!("{method}\n{key}\n{exp}\n{mime}").as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn url(&self, method: &str, key: &str, mime: &str) -> Result<String> {
        let exp = chrono::Utc::now().timestamp() + url_ttl().as_secs() as i64;
        let mut u = self.base.join("files/")?.join(key)?;
        u.query_pairs_mut()
            .append_pair("m", method)
            .append_pair("exp", &exp.to_string())
            .append_pair("ct", mime)
            .append_pair("sig", &self.sign(method, key, exp, mime));
        Ok(u.to_string())
    }

    /// Checks a signed URL presented to `/files/<key>`.
    pub fn verify(&self, method: &str, key: &str, exp: i64, mime: &str, sig: &str) -> bool {
        if exp < chrono::Utc::now().timestamp() { return false; }
        let expected = self.sign(method, key, exp, mime);
        // constant-time comparison
        expected.len() == sig.len() && expected.bytes().zip(sig.bytes()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0
    }

    pub fn path(&self, key: &str) -> Result<PathBuf> { Ok(self.root.join(safe_key(key)?)) }
}

#[async_trait]
impl ObjectStore for DiskStore {
    fn kind(&self) -> StoreKind { StoreKind::Disk }
    async fn presign_put(&self, key: &str, mime: &str) -> Result<String> { self.url("PUT", safe_key(key)?, mime) }
    async fn presign_get(&self, key: &str) -> Result<String> {
        let mime = tokio::fs::read_to_string(self.path(key)?.with_extension("mime")).await.unwrap_or_default();
        self.url("GET", safe_key(key)?, mime.trim())
    }
    async fn get(&self, key: &str) -> Result<Bytes> { Ok(tokio::fs::read(self.path(key)?).await?.into()) }
    async fn put(&self, key: &str, data: Bytes, mime: &str) -> Result<()> {
        let p = self.path(key)?;
        if let Some(dir) = p.parent() { tokio::fs::create_dir_all(dir).await?; }
        tokio::fs::write(&p, &data).await?;
        tokio::fs::write(p.with_extension("mime"), mime).await?;
        Ok(())
    }
}

// ---------------- S3-compatible store (internet) ----------------

pub struct S3Store {
    bucket: String,
    client: aws_sdk_s3::Client,
    /// Separate client whose endpoint is the public host, so presigned URLs work from phones.
    presigner: aws_sdk_s3::Client,
}

impl S3Store {
    pub async fn new(cfg: &Config) -> Result<Self> {
        let shared = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(cfg.s3_region.clone()))
            .load()
            .await;
        let make = |endpoint: Option<&String>| {
            let mut b = aws_sdk_s3::config::Builder::from(&shared).force_path_style(true);
            if let Some(ep) = endpoint { b = b.endpoint_url(ep); }
            aws_sdk_s3::Client::from_conf(b.build())
        };
        let client = make(cfg.s3_endpoint.as_ref());
        let presigner = make(cfg.s3_public_endpoint.as_ref().or(cfg.s3_endpoint.as_ref()));
        Ok(Self { bucket: cfg.s3_bucket.clone(), client, presigner })
    }

    fn presign_cfg() -> Result<aws_sdk_s3::presigning::PresigningConfig> {
        Ok(aws_sdk_s3::presigning::PresigningConfig::expires_in(url_ttl())?)
    }
}

#[async_trait]
impl ObjectStore for S3Store {
    fn kind(&self) -> StoreKind { StoreKind::S3 }

    async fn presign_put(&self, key: &str, mime: &str) -> Result<String> {
        let req = self.presigner.put_object().bucket(&self.bucket).key(safe_key(key)?).content_type(mime)
            .presigned(Self::presign_cfg()?).await?;
        Ok(req.uri().to_string())
    }

    async fn presign_get(&self, key: &str) -> Result<String> {
        let req = self.presigner.get_object().bucket(&self.bucket).key(safe_key(key)?)
            .presigned(Self::presign_cfg()?).await?;
        Ok(req.uri().to_string())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let out = self.client.get_object().bucket(&self.bucket).key(key).send().await?;
        Ok(out.body.collect().await?.into_bytes())
    }

    async fn put(&self, key: &str, data: Bytes, mime: &str) -> Result<()> {
        self.client.put_object().bucket(&self.bucket).key(key).content_type(mime)
            .body(aws_sdk_s3::primitives::ByteStream::from(data)).send().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the S3 code against a real S3-compatible store (the internet profile uses SeaweedFS):
    /// what the worker does (put / get) and what the apps do (presigned upload and download).
    ///
    /// Needs a store, so it is skipped by default. To run it, set FILE_STORE=s3, S3_ENDPOINT,
    /// S3_BUCKET, AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY, then:
    ///     cargo test --lib s3_store -- --ignored
    #[tokio::test]
    #[ignore = "needs an S3-compatible store; see the comment"]
    async fn s3_store_round_trip_with_presigned_links() {
        let cfg = Config::from_env().expect("configuration");
        assert_eq!(cfg.store, StoreKind::S3, "set FILE_STORE=s3");
        let store = S3Store::new(&cfg).await.expect("S3 client");
        let http = reqwest::Client::new();
        let id = ulid::Ulid::new();

        // Worker side: the bucket is created on the first upload; thumbnails are written and read.
        let direct = format!("thumbs/uploads/test/{id}-report.pdf.jpg");
        store.put(&direct, Bytes::from_static(b"thumbnail bytes"), "image/jpeg").await.expect("direct put");
        assert_eq!(&store.get(&direct).await.expect("direct get")[..], b"thumbnail bytes");

        // App side: upload through a presigned link (a file bigger than one internal chunk)...
        let key = format!("uploads/test/{id}-scan 1.jpg");
        let body: Vec<u8> = (0..6_000_000u32).map(|i| (i % 251) as u8).collect();
        let put_url = store.presign_put(&key, "image/jpeg").await.expect("presign put");
        let put = http.put(&put_url).header("content-type", "image/jpeg").body(body.clone()).send().await.expect("upload");
        assert!(put.status().is_success(), "presigned upload: {} {}", put.status(), put.text().await.unwrap_or_default());

        // ...then download through a presigned link, and read it back as the worker would.
        let get_url = store.presign_get(&key).await.expect("presign get");
        let got = http.get(&get_url).send().await.expect("download");
        assert!(got.status().is_success(), "presigned download: {}", got.status());
        assert_eq!(got.bytes().await.unwrap().len(), body.len());
        assert_eq!(&store.get(&key).await.expect("get uploaded")[..], &body[..]);

        // Links are the only way in: no signature, a changed signature or another object are refused.
        let plain = get_url.split('?').next().unwrap().to_string();
        assert_eq!(http.get(&plain).send().await.unwrap().status(), 403, "anonymous read must be refused");
        let forged = get_url.replacen("X-Amz-Signature=", "X-Amz-Signature=0", 1);
        assert_eq!(http.get(&forged).send().await.unwrap().status(), 403, "a changed signature must be refused");
        let other = get_url.replacen("scan%201.jpg", "scan%202.jpg", 1);
        assert_ne!(other, get_url, "test is out of date: the key is not in the link as expected");
        assert_eq!(http.get(&other).send().await.unwrap().status(), 403, "a link is valid for its own object only");
        // An upload link can't be used to send a different type than the one it was made for.
        let wrong_type = http.put(&put_url).header("content-type", "application/pdf").body(vec![1u8; 10]).send().await.unwrap();
        assert_eq!(wrong_type.status(), 403, "the content type is part of the signed upload");
    }
}
