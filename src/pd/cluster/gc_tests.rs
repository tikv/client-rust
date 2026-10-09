// Copyright 2026 TiKV Project Authors. Licensed under Apache-2.0.

use std::convert::Infallible;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::task::{Context, Poll};

use tonic::body::BoxBody;
use tonic::codegen::{http, BoxFuture, Service};
use tonic::server::NamedService;

use super::*;

type GcCall = (Option<apipb::KeyspaceIdentity>, u64);

#[derive(Clone, Default)]
struct GcService {
    calls: Arc<Mutex<Vec<GcCall>>>,
    // 0: accepted; 1: already newer; 2: PD header error; 3: unsupported RPC.
    reply: Arc<AtomicU8>,
}

impl GcService {
    fn response(&self, safe_point: u64) -> GrpcResult<(pdpb::ResponseHeader, u64)> {
        let mut header = pdpb::ResponseHeader {
            cluster_id: 42,
            ..Default::default()
        };
        match self.reply.load(Ordering::SeqCst) {
            1 => Ok((header, safe_point + 1)),
            2 => {
                header.error = Some(pdpb::Error {
                    message: "scoped GC rejected".into(),
                    ..Default::default()
                });
                Ok((header, safe_point))
            }
            3 => Err(tonic::Status::unimplemented("scoped GC unavailable")),
            _ => Ok((header, safe_point)),
        }
    }
}

struct ScopedGc(GcService);

impl tonic::server::UnaryService<pdpb::UpdateGcSafePointV2Request> for ScopedGc {
    type Response = pdpb::UpdateGcSafePointV2Response;
    type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;

    fn call(&mut self, request: Request<pdpb::UpdateGcSafePointV2Request>) -> Self::Future {
        let service = self.0.clone();
        Box::pin(async move {
            let request = request.into_inner();
            assert_eq!(request.header.unwrap().cluster_id, 42);
            let Some(pdpb::update_gc_safe_point_v2_request::Keyspace::KeyspaceIdentity(identity)) =
                request.keyspace
            else {
                panic!("V3 GC must send the complete identity");
            };
            service
                .calls
                .lock()
                .unwrap()
                .push((Some(identity), request.safe_point));
            let (header, new_safe_point) = service.response(request.safe_point)?;
            Ok(tonic::Response::new(pdpb::UpdateGcSafePointV2Response {
                header: Some(header),
                new_safe_point,
            }))
        })
    }
}

struct GlobalGc(GcService);

impl tonic::server::UnaryService<pdpb::UpdateGcSafePointRequest> for GlobalGc {
    type Response = pdpb::UpdateGcSafePointResponse;
    type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;

    fn call(&mut self, request: Request<pdpb::UpdateGcSafePointRequest>) -> Self::Future {
        let service = self.0.clone();
        Box::pin(async move {
            let request = request.into_inner();
            assert_eq!(request.header.unwrap().cluster_id, 42);
            service
                .calls
                .lock()
                .unwrap()
                .push((None, request.safe_point));
            let (header, new_safe_point) = service.response(request.safe_point)?;
            Ok(tonic::Response::new(pdpb::UpdateGcSafePointResponse {
                header: Some(header),
                new_safe_point,
            }))
        })
    }
}

impl Service<http::Request<BoxBody>> for GcService {
    type Response = http::Response<BoxBody>;
    type Error = Infallible;
    type Future = BoxFuture<Self::Response, Self::Error>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<BoxBody>) -> Self::Future {
        let service = self.clone();
        Box::pin(async move {
            match request.uri().path() {
                "/pdpb.PD/UpdateGCSafePointV2" => {
                    let codec = tonic::codec::ProstCodec::default();
                    Ok(tonic::server::Grpc::new(codec)
                        .unary(ScopedGc(service), request)
                        .await)
                }
                "/pdpb.PD/UpdateGCSafePoint" => {
                    let codec = tonic::codec::ProstCodec::default();
                    Ok(tonic::server::Grpc::new(codec)
                        .unary(GlobalGc(service), request)
                        .await)
                }
                _ => Ok(http::Response::builder()
                    .header("grpc-status", "12")
                    .header("content-type", "application/grpc")
                    .body(tonic::body::empty_body())
                    .unwrap()),
            }
        })
    }
}

impl NamedService for GcService {
    const NAME: &'static str = "pdpb.PD";
}

#[tokio::test]
async fn gc_safepoint_rpc_preserves_scope_and_never_falls_back() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let service = GcService::default();
    let incoming = futures::stream::unfold(listener, |listener| async move {
        let result = listener.accept().await.map(|(stream, _)| stream);
        Some((result, listener))
    });
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(service.clone())
            .serve_with_incoming(incoming),
    );
    let channel = Channel::from_shared(format!("http://{address}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let client = pdpb::pd_client::PdClient::new(channel.clone());
    let tso = TimestampOracle::new(42, &client, Arc::new(SecurityManager::default())).unwrap();
    let mut cluster = Cluster {
        id: 42,
        client,
        keyspace_client: keyspacepb::keyspace_client::KeyspaceClient::new(channel),
        members: pdpb::GetMembersResponse::default(),
        tso,
    };
    let timeout = Duration::from_secs(3);
    for namespace_id in [7, 19] {
        let identity = apipb::KeyspaceIdentity {
            namespace_id,
            keyspace_id: 42,
        };
        assert!(cluster
            .update_safepoint_with_identity(100, Some(identity.clone()), timeout)
            .await
            .unwrap());
        assert_eq!(
            service.calls.lock().unwrap().last(),
            Some(&(Some(identity), 100))
        );
    }
    for keyspace in [
        crate::request::Keyspace::Disable,
        crate::request::Keyspace::Enable { keyspace_id: 42 },
        crate::request::Keyspace::ApiV2NoPrefix,
    ] {
        assert!(cluster
            .update_safepoint_with_identity(100, keyspace.v3_identity(), timeout)
            .await
            .unwrap());
    }
    let identity = apipb::KeyspaceIdentity {
        namespace_id: 7,
        keyspace_id: 42,
    };
    service.reply.store(1, Ordering::SeqCst);
    assert!(!cluster
        .update_safepoint_with_identity(100, Some(identity.clone()), timeout)
        .await
        .unwrap());
    for reply in [2, 3] {
        service.reply.store(reply, Ordering::SeqCst);
        cluster
            .update_safepoint_with_identity(100, Some(identity.clone()), timeout)
            .await
            .unwrap_err();
    }
    let calls = service.calls.lock().unwrap();
    assert_eq!(calls.len(), 8);
    assert_eq!(
        calls
            .iter()
            .filter(|(identity, _)| identity.is_none())
            .count(),
        3
    );
    drop(calls);
    server.abort();
}

#[tokio::test]
async fn unsupported_pd_implementation_does_not_fall_back_to_global_gc() {
    use crate::pd::PdClient;
    let pd = Arc::new(crate::mock::MockPdClient::new(
        crate::mock::MockKvClient::default(),
    ));
    // The mock's global update panics; the default scoped method must reject
    // the request without reaching it.
    let error = pd
        .update_safepoint_with_identity(
            100,
            Some(apipb::KeyspaceIdentity {
                namespace_id: 7,
                keyspace_id: 42,
            }),
        )
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("scoped GC safe point updates are not supported"));
}
