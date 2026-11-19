//! The generated kvproto modules are public: applications can name the messages and
//! build the gRPC clients this crate does not wrap. No cluster is needed.

use tikv_client::proto::cdcpb;
use tikv_client::proto::kvrpcpb;
use tikv_client::proto::pdpb;
use tikv_client::proto::prost::Message;
use tikv_client::proto::tonic::transport::Channel;
use tikv_client::proto::tonic::transport::Endpoint;

#[test]
fn generated_messages_are_public_and_encode_with_the_reexported_prost() {
    let request = pdpb::UpdateServiceGcSafePointRequest {
        header: Some(pdpb::RequestHeader {
            cluster_id: 7,
            ..Default::default()
        }),
        service_id: b"gc_worker".to_vec(),
        ttl: i64::MAX,
        safe_point: 42,
    };
    let decoded =
        pdpb::UpdateServiceGcSafePointRequest::decode(request.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, request);

    // The re-exported `ProtoLockInfo` is the same type as the generated one.
    let lock: tikv_client::ProtoLockInfo = kvrpcpb::LockInfo::default();
    assert!(lock.secondaries.is_empty());
}

#[tokio::test]
async fn generated_clients_build_on_the_reexported_tonic() {
    // A lazy channel never dials, so this needs no server.
    let channel: Channel = Endpoint::from_static("http://127.0.0.1:1").connect_lazy();
    let _pd = pdpb::pd_client::PdClient::new(channel.clone());
    let _cdc = cdcpb::change_data_client::ChangeDataClient::new(channel)
        .max_decoding_message_size(64 << 20);
}
