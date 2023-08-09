use std::{net::SocketAddr, str::FromStr};

use crate::{Error, Result};
use url::Url;

use trust_dns_resolver::{
    config::{NameServerConfig, Protocol, ResolverConfig, ResolverOpts},
    Name, Resolver,
};

pub async fn custom_dns(
    target: &str,
    dns_addr: &str,
    search_domain: &Vec<String>,
) -> Result<String> {
    let server: SocketAddr = dns_addr.parse().map_err(|e| Error::InternalError {
        message: format!("dns server error: {}", e),
    })?;
    let mut search_names: Vec<Name> = Vec::new();
    for d in search_domain {
        let n = Name::from_str(d.as_str()).map_err(|e| Error::InternalError {
            message: format!("dns search domain error: {}", e),
        })?;
        search_names.push(n);
    }
    let resolver_config = ResolverConfig::from_parts(
        None,
        search_names,
        vec![NameServerConfig {
            socket_addr: server,
            protocol: Protocol::Udp,
            tls_dns_name: None,
        }],
    );
    let resolver = Resolver::new(resolver_config, ResolverOpts::default()).map_err(|e| {
        Error::InternalError {
            message: format!("dns resolver error: {}", e),
        }
    })?;
    let mut url = Url::parse(target).map_err(|e| Error::InternalError {
        message: format!("url parse error: {}", e),
    })?;
    let hostname = url.host_str().ok_or(Error::InternalError {
        message: format!("url parse error: {}", url),
    })?;
    let ip = resolver
        .lookup_ip(hostname)
        .map_err(|e| Error::InternalError {
            message: format!("dns resolve error: {}", e),
        })?
        .iter()
        .next()
        .ok_or(Error::InternalError {
            message: format!("can't resolve hostname {}", hostname),
        })?
        .to_string();
    url.set_host(Some(ip.as_str()))
        .map_err(|e| Error::InternalError {
            message: format!("url parse error: {}", e),
        })?;
    Ok(String::from(url))
}
