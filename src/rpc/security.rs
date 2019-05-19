// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use grpcio::{Channel, ChannelBuilder, ChannelCredentialsBuilder, Environment};
use log::*;

use crate::Result;

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

#[derive(Default)]
pub struct SecurityManager {
    ca: Vec<u8>,
    cert: Vec<u8>,
    key: PathBuf,
}

impl SecurityManager {
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

    pub fn connect<Factory, Client>(
        &self,
        env: Arc<Environment>,
        addr: &str,
        factory: Factory,
    ) -> Result<Client>
    where
        Factory: FnOnce(Channel) -> Client,
    {
        info!("connect to rpc server at endpoint: {:?}", addr);
        let addr = addr
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let cb = ChannelBuilder::new(env)
            .keepalive_time(Duration::from_secs(10))
            .keepalive_timeout(Duration::from_secs(3));

        let channel = if self.ca.is_empty() {
            cb.connect(addr)
        } else {
            let cred = ChannelCredentialsBuilder::new()
                .root_cert(self.ca.clone())
                .cert(self.cert.clone(), load_pem_file("private key", &self.key)?)
                .build();
            cb.secure_connect(addr, cred)
        };

        Ok(factory(channel))
    }
}

#[cfg(test)]
mod tests {
    extern crate tempdir;
    use super::*;

    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;

    use self::tempdir::TempDir;

    #[test]
    fn test_security() {
        let temp = TempDir::new("test_cred").unwrap();
        let example_ca = temp.path().join("ca");
        let example_cert = temp.path().join("cert");
        let example_pem = temp.path().join("key");
        for (id, f) in (&[&example_ca, &example_cert, &example_pem])
            .iter()
            .enumerate()
        {
            File::create(f).unwrap().write_all(&[id as u8]).unwrap();
        }
        let cert_path: PathBuf = format!("{}", example_cert.display()).into();
        let key_path: PathBuf = format!("{}", example_pem.display()).into();
        let ca_path: PathBuf = format!("{}", example_ca.display()).into();
        let mgr = SecurityManager::load(&ca_path, &cert_path, &key_path).unwrap();
        assert_eq!(mgr.ca, vec![0]);
        assert_eq!(mgr.cert, vec![1]);
        let key = load_pem_file("private key", &key_path).unwrap();
        assert_eq!(key, vec![2]);
    }
}
