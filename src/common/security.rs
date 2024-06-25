// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

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
    /// The PEM encoding of the server’s CA certificates.
    ca: Vec<u8>,
    /// The PEM encoding of the server’s certificate chain.
    cert: Vec<u8>,
    /// The path to the file that contains the PEM encoding of the server’s private key.
    key: PathBuf,
}

impl SecurityManager {
    /// Load TLS configuration from files.
    pub fn load(
        ca_path: impl AsRef<Path>,
        cert_path: impl AsRef<Path>,
        key_path: impl Into<PathBuf>,
    ) -> Result<SecurityManager> {
        let key_path = key_path.into();
        check_pem_file("private key", &key_path)?;
        Ok(SecurityManager {
            ca: load_pem_file("ca", ca_path.as_ref())?,
            cert: load_pem_file("certificate", cert_path.as_ref())?,
            key: key_path,
        })
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
        let channel = if !self.ca.is_empty() {
            self.tls_channel(addr).await?
        } else {
            self.default_channel(addr).await?
        };
        let ch = channel.connect().await?;

        Ok(factory(ch))
    }

    async fn tls_channel(&self, addr: &str) -> Result<Endpoint> {
        let addr = "https://".to_string() + &SCHEME_REG.replace(addr, "");
        let builder = self.endpoint(addr.to_string())?;
        let tls = ClientTlsConfig::new()
            .ca_certificate(Certificate::from_pem(&self.ca))
            .identity(Identity::from_pem(
                &self.cert,
                load_pem_file("private key", &self.key)?,
            ));
        let builder = builder.tls_config(tls)?;
        Ok(builder)
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
        assert_eq!(mgr.ca, vec![0]);
        assert_eq!(mgr.cert, vec![1]);
        let key = load_pem_file("private key", &key_path).unwrap();
        assert_eq!(key, vec![2]);
    }
}
