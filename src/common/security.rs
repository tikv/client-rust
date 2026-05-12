// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;

use log::info;
use regex::Regex;
use tonic::transport::Channel;
use tonic::transport::ClientTlsConfig;
use tonic::transport::Identity;
use tonic::transport::{Certificate, Endpoint};

use crate::internal_err;
use crate::Result;

lazy_static::lazy_static! {
    static ref SCHEME_REG: Regex = Regex::new(r"^\s*(https?://)").unwrap();
}

fn check_pem_file(tag: &str, path: &Path) -> Result<File> {
    File::open(path)
        .map_err(|e| internal_err!("failed to open {} to load {}: {:?}", path.display(), tag, e))
}

fn load_pem_file(tag: &str, path: &Path) -> Result<Vec<u8>> {
    let mut file = check_pem_file(tag, path)?;
    let mut key = vec![];
    file.read_to_end(&mut key)
        .map_err(|e| {
            internal_err!(
                "failed to load {} from path {}: {:?}",
                tag,
                path.display(),
                e
            )
        })
        .map(|_| key)
}

/// Manages the TLS protocol
#[derive(Default)]
pub struct SecurityManager {
    /// The path to the PEM encoding of the server’s CA certificates.
    ca_path: Option<PathBuf>,
    /// The path to the PEM encoding of the server’s certificate chain.
    cert_path: Option<PathBuf>,
    /// The path to the file that contains the PEM encoding of the server’s private key.
    key_path: Option<PathBuf>,
}

impl SecurityManager {
    /// Load TLS configuration from files.
    pub fn load(
        ca_path: impl AsRef<Path>,
        cert_path: impl AsRef<Path>,
        key_path: impl Into<PathBuf>,
    ) -> Result<SecurityManager> {
        let ca_path = ca_path.as_ref().to_path_buf();
        let cert_path = cert_path.as_ref().to_path_buf();
        let key_path = key_path.into();
        check_pem_file("ca", &ca_path)?;
        check_pem_file("certificate", &cert_path)?;
        check_pem_file("private key", &key_path)?;
        Ok(SecurityManager {
            ca_path: Some(ca_path),
            cert_path: Some(cert_path),
            key_path: Some(key_path),
        })
    }

    pub(crate) fn tls_configured(&self) -> bool {
        self.ca_path.is_some()
    }

    pub(crate) async fn connection_cache_key(&self) -> Result<Option<u64>> {
        if !self.tls_configured() {
            return Ok(None);
        }

        let mut hasher = DefaultHasher::new();
        file_signature(self.ca_path.as_ref().expect("tls_configured checked"))
            .await?
            .hash(&mut hasher);
        file_signature(self.cert_path.as_ref().expect("tls_configured checked"))
            .await?
            .hash(&mut hasher);
        file_signature(self.key_path.as_ref().expect("tls_configured checked"))
            .await?
            .hash(&mut hasher);
        Ok(Some(hasher.finish()))
    }

    /// Connect to gRPC server using TLS connection. If TLS is not configured, use normal connection.
    pub async fn connect<Factory, Client>(
        &self,
        // env: Arc<Environment>,
        addr: &str,
        factory: Factory,
    ) -> Result<Client>
    where
        Factory: FnOnce(Channel) -> Client,
    {
        info!("connect to rpc server at endpoint: {:?}", addr);
        let channel = if self.tls_configured() {
            self.tls_channel(addr).await?
        } else {
            self.default_channel(addr).await?
        };
        let ch = channel.connect().await?;

        Ok(factory(ch))
    }

    async fn tls_channel(&self, addr: &str) -> Result<Endpoint> {
        let (ca, cert, key) = self.load_tls_materials()?;
        let addr = "https://".to_string() + &SCHEME_REG.replace(addr, "");
        let builder = self.endpoint(addr.to_string())?;
        let tls = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(ca))
            .identity(Identity::from_pem(cert, key));
        let builder = builder.tls_config(tls)?;
        Ok(builder)
    }

    fn load_tls_materials(&self) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let ca_path = self
            .ca_path
            .as_ref()
            .ok_or_else(|| internal_err!("TLS is not configured"))?;
        let cert_path = self
            .cert_path
            .as_ref()
            .ok_or_else(|| internal_err!("TLS is not configured"))?;
        let key_path = self
            .key_path
            .as_ref()
            .ok_or_else(|| internal_err!("TLS is not configured"))?;

        Ok((
            load_pem_file("ca", ca_path)?,
            load_pem_file("certificate", cert_path)?,
            load_pem_file("private key", key_path)?,
        ))
    }

    async fn default_channel(&self, addr: &str) -> Result<Endpoint> {
        let addr = "http://".to_string() + &SCHEME_REG.replace(addr, "");
        self.endpoint(addr)
    }

    fn endpoint(&self, addr: String) -> Result<Endpoint> {
        let endpoint = Channel::from_shared(addr)?
            .tcp_keepalive(Some(Duration::from_secs(10)))
            .keep_alive_timeout(Duration::from_secs(3));
        Ok(endpoint)
    }
}

async fn file_signature(path: &Path) -> Result<(u64, Option<u128>)> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| internal_err!("failed to stat {}: {:?}", path.display(), e))?;
    let modified = metadata.modified().ok().and_then(|t: SystemTime| {
        t.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
    });
    Ok((metadata.len(), modified))
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;

    use tempfile;

    use super::*;

    #[test]
    fn test_security() {
        let temp = tempfile::tempdir().unwrap();
        let example_ca = temp.path().join("ca");
        let example_cert = temp.path().join("cert");
        let example_pem = temp.path().join("key");
        for (id, f) in [&example_ca, &example_cert, &example_pem]
            .iter()
            .enumerate()
        {
            File::create(f).unwrap().write_all(&[id as u8]).unwrap();
        }
        let cert_path: PathBuf = format!("{}", example_cert.display()).into();
        let key_path: PathBuf = format!("{}", example_pem.display()).into();
        let ca_path: PathBuf = format!("{}", example_ca.display()).into();
        let mgr = SecurityManager::load(ca_path, cert_path, &key_path).unwrap();
        assert!(mgr.tls_configured());
        let (ca, cert, key) = mgr.load_tls_materials().unwrap();
        assert_eq!(ca, vec![0]);
        assert_eq!(cert, vec![1]);
        assert_eq!(key, vec![2]);
    }

    #[tokio::test]
    async fn test_security_reload() {
        let temp = tempfile::tempdir().unwrap();
        let example_ca = temp.path().join("ca");
        let example_cert = temp.path().join("cert");
        let example_pem = temp.path().join("key");
        for (id, f) in [&example_ca, &example_cert, &example_pem]
            .iter()
            .enumerate()
        {
            File::create(f).unwrap().write_all(&[id as u8]).unwrap();
        }

        let mgr = SecurityManager::load(&example_ca, &example_cert, &example_pem).unwrap();
        let first = mgr.load_tls_materials().unwrap();
        let key1 = mgr.connection_cache_key().await.unwrap();

        File::create(&example_ca)
            .unwrap()
            .write_all(&[9, 9])
            .unwrap();
        File::create(&example_cert)
            .unwrap()
            .write_all(&[8, 8, 8])
            .unwrap();
        File::create(&example_pem)
            .unwrap()
            .write_all(&[7, 7, 7, 7])
            .unwrap();

        let second = mgr.load_tls_materials().unwrap();
        let key2 = mgr.connection_cache_key().await.unwrap();
        assert_ne!(first, second);
        assert_eq!(second.0, vec![9, 9]);
        assert_eq!(second.1, vec![8, 8, 8]);
        assert_eq!(second.2, vec![7, 7, 7, 7]);
        assert_ne!(key1, key2);
    }
}
