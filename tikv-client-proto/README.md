# TiKV client protobuf definitions

This crate builds Rust protobufs required by the TiKV client.

The protobuf definitions are in proto and include. These are copied from the [kvproto repo](https://github.com/pingcap/kvproto). They are copied because the kvproto crate is difficult to publish.

To update the protos, copy them all from that repo. They will be rebuilt automatically when you build the client (or this crate).
