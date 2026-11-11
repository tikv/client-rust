// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::path::PathBuf;

use clap::crate_version;
use clap::Arg;
use clap::ArgAction;
use clap::Command;

pub struct CommandArgs {
    pub pd: Vec<String>,
    pub ca: Option<PathBuf>,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

pub fn parse_args(app_name: &str) -> CommandArgs {
    let matches = Command::new(app_name.to_owned())
        .version(crate_version!())
        .author("The TiKV Project Authors")
        .arg(
            Arg::new("pd")
                .long("pd")
                .aliases(["pd-endpoint", "pd-endpoints"])
                .value_name("PD_URL")
                .help("Sets PD endpoints")
                .long_help("Sets PD endpoints. Uses `,` to separate multiple PDs")
                .action(ArgAction::Append)
                .num_args(1..)
                .value_delimiter(',')
                .required(false)
                .default_value("localhost:2379"),
        )
        // A cyclic dependency between CA, cert and key is made
        // to ensure that no security options are missing.
        .arg(
            Arg::new("ca")
                .long("ca")
                .value_name("CA_PATH")
                .help("Sets the CA")
                .long_help("Sets the CA. Must be used with --cert and --key")
                .requires("cert"),
        )
        .arg(
            Arg::new("cert")
                .long("cert")
                .value_name("CERT_PATH")
                .help("Sets the certificate")
                .long_help("Sets the certificate. Must be used with --ca and --key")
                .requires("key"),
        )
        .arg(
            Arg::new("key")
                .long("key")
                .alias("private-key")
                .value_name("KEY_PATH")
                .help("Sets the private key")
                .long_help("Sets the private key. Must be used with --ca and --cert")
                .requires("ca"),
        )
        .get_matches();

    CommandArgs {
        pd: matches.get_many::<String>("pd").unwrap().cloned().collect(),
        ca: matches.get_one::<String>("ca").map(PathBuf::from),
        cert: matches.get_one::<String>("cert").map(PathBuf::from),
        key: matches.get_one::<String>("key").map(PathBuf::from),
    }
}
