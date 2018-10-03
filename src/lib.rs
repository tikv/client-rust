#![crate_type = "lib"]
#![feature(box_syntax)]
#![feature(fnbox)]
#![recursion_limit = "200"]

#[macro_use]
extern crate log;
extern crate byteorder;
extern crate futures;
extern crate futures_cpupool;
extern crate fxhash;
extern crate grpcio as grpc;
extern crate indexmap;
extern crate kvproto;
extern crate protobuf;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate backtrace;
extern crate chrono;
extern crate crc;
extern crate crossbeam;
extern crate serde_json;
extern crate time;
#[macro_use]
extern crate crossbeam_channel;
extern crate prometheus;
extern crate prometheus_static_metric;
extern crate tokio_core;
extern crate tokio_timer;
#[macro_use]
extern crate quick_error;
extern crate mio;
extern crate url;
#[macro_use]
extern crate lazy_static;
#[cfg(target_os = "linux")]
extern crate libc;

#[macro_use]
pub mod util;
pub mod pd;
