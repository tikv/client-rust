use fxhash::FxHashSet as HashSet;
use std::result;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;

use futures::future::{loop_fn, ok, Loop};
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::sync::oneshot;
use futures::task::Task;
use futures::Future;
use grpc::{CallOption, ChannelBuilder, ClientDuplexReceiver, ClientDuplexSender, Environment};
use kvproto::pdpb::{
    GetMembersRequest, GetMembersResponse, ResponseHeader, TsoRequest, TsoResponse,
};
use kvproto::pdpb_grpc::PdClient;
use tokio_timer::timer::Handle;

use super::{Error, PdFuture, PdTimestamp, Result, REQUEST_TIMEOUT};
use util::security::SecurityManager;
use util::timer::GLOBAL_TIMER_HANDLE;
use util::{Either, HandyRwLock};

macro_rules! box_err {
    ($e:expr) => ({
        use std::error::Error;
        let e: Box<Error + Sync + Send> = format!("[{}:{}]: {}", file!(), line!(),  $e).into();
        e.into()
    });
    ($f:tt, $($arg:expr),+) => ({
        box_err!(format!($f, $($arg),+))
    });
}

pub struct LeaderClient {
    env: Arc<Environment>,
    pub client: PdClient,
    pub members: GetMembersResponse,
    security_mgr: Arc<SecurityManager>,
    pub on_reconnect: Option<Box<Fn() + Sync + Send + 'static>>,
    pub tso_sender: Either<Option<ClientDuplexSender<TsoRequest>>, UnboundedSender<TsoRequest>>,
    pub tso_receiver: Either<Option<ClientDuplexReceiver<TsoResponse>>, Task>,

    pub tso_requests: UnboundedReceiver<oneshot::Sender<PdTimestamp>>,
    pub tso_requests_sender: UnboundedSender<oneshot::Sender<PdTimestamp>>,

    last_update: Instant,
}

impl LeaderClient {
    pub fn new(
        env: Arc<Environment>,
        security_mgr: Arc<SecurityManager>,
        client: PdClient,
        members: GetMembersResponse,
    ) -> LeaderClient {
        let (tx, rx) = client.tso().unwrap();
        let (tso_sender, tso_receiver) = unbounded();
        LeaderClient {
            env,
            tso_sender: Either::Left(Some(tx)),
            tso_receiver: Either::Left(Some(rx)),
            client,
            members,
            security_mgr,
            on_reconnect: None,

            last_update: Instant::now(),

            tso_requests: tso_receiver,
            tso_requests_sender: tso_sender,
        }
    }
}

pub const RECONNECT_INTERVAL_SEC: u64 = 1; // 1s

/// The context of sending requets.
pub struct Request<Req, Resp, F> {
    reconnect_count: usize,
    request_sent: usize,

    leader: Arc<RwLock<LeaderClient>>,
    timer: Handle,

    req: Req,
    resp: Option<Result<Resp>>,
    func: F,
}

const MAX_REQUEST_COUNT: usize = 3;

impl<Req, Resp, F> Request<Req, Resp, F>
where
    Req: Clone + Send + 'static,
    Resp: Send + 'static,
    F: FnMut(&RwLock<LeaderClient>, Req) -> PdFuture<Resp> + Send + 'static,
{
    pub fn new(
        req: Req,
        func: F,
        leader: Arc<RwLock<LeaderClient>>,
        retry: usize,
    ) -> Request<Req, Resp, F> {
        Request {
            reconnect_count: retry,
            request_sent: 0,
            leader,
            timer: GLOBAL_TIMER_HANDLE.clone(),
            req,
            resp: None,
            func,
        }
    }

    fn reconnect_if_needed(mut self) -> Box<Future<Item = Self, Error = Self> + Send> {
        debug!("reconnect remains: {}", self.reconnect_count);

        if self.request_sent < MAX_REQUEST_COUNT {
            return Box::new(ok(self));
        }

        // Updating client.
        self.reconnect_count -= 1;

        // FIXME: should not block the core.
        warn!("updating PD client, block the tokio core");
        match reconnect(&self.leader) {
            Ok(_) => {
                self.request_sent = 0;
                Box::new(ok(self))
            }
            Err(_) => Box::new(
                self.timer
                    .delay(Instant::now() + Duration::from_secs(RECONNECT_INTERVAL_SEC))
                    .then(|_| Err(self)),
            ),
        }
    }

    fn send_and_receive(mut self) -> Box<Future<Item = Self, Error = Self> + Send> {
        self.request_sent += 1;
        debug!("request sent: {}", self.request_sent);
        let r = self.req.clone();

        Box::new(ok(self).and_then(|mut ctx| {
            let req = (ctx.func)(&ctx.leader, r);
            req.then(|resp| match resp {
                Ok(resp) => {
                    ctx.resp = Some(Ok(resp));
                    Ok(ctx)
                }
                Err(err) => {
                    error!("request failed: {:?}", err);
                    Err(ctx)
                }
            })
        }))
    }

    fn break_or_continue(ctx: result::Result<Self, Self>) -> Result<Loop<Self, Self>> {
        let ctx = match ctx {
            Ok(ctx) | Err(ctx) => ctx,
        };
        let done = ctx.reconnect_count == 0 || ctx.resp.is_some();
        if done {
            Ok(Loop::Break(ctx))
        } else {
            Ok(Loop::Continue(ctx))
        }
    }

    fn post_loop(ctx: Result<Self>) -> Result<Resp> {
        let ctx = ctx.expect("end loop with Ok(_)");
        ctx.resp.unwrap_or_else(|| Err(box_err!("fail to request")))
    }

    /// Returns a Future, it is resolves once a future returned by the closure
    /// is resolved successfully, otherwise it repeats `retry` times.
    pub fn execute(self) -> PdFuture<Resp> {
        let ctx = self;
        Box::new(
            loop_fn(ctx, |ctx| {
                ctx.reconnect_if_needed()
                    .and_then(Self::send_and_receive)
                    .then(Self::break_or_continue)
            }).then(Self::post_loop),
        )
    }
}

pub fn validate_endpoints(
    env: &Arc<Environment>,
    endpoints: &[&str],
    security_mgr: &SecurityManager,
) -> Result<(PdClient, GetMembersResponse)> {
    let len = endpoints.len();
    let mut endpoints_set = HashSet::with_capacity_and_hasher(len, Default::default());

    let mut members = None;
    let mut cluster_id = None;
    for ep in endpoints {
        if !endpoints_set.insert(ep) {
            return Err(box_err!("duplicate PD endpoint {}", ep));
        }

        let (_, resp) = match connect(Arc::clone(&env), security_mgr, ep) {
            Ok(resp) => resp,
            // Ignore failed PD node.
            Err(e) => {
                error!("PD endpoint {} failed to respond: {:?}", ep, e);
                continue;
            }
        };

        // Check cluster ID.
        let cid = resp.get_header().get_cluster_id();
        if let Some(sample) = cluster_id {
            if sample != cid {
                return Err(box_err!(
                    "PD response cluster_id mismatch, want {}, got {}",
                    sample,
                    cid
                ));
            }
        } else {
            cluster_id = Some(cid);
        }
        // TODO: check all fields later?

        if members.is_none() {
            members = Some(resp);
        }
    }

    match members {
        Some(members) => {
            let (client, members) = try_connect_leader(&env, security_mgr, &members)?;
            info!("All PD endpoints are consistent: {:?}", endpoints);
            Ok((client, members))
        }
        _ => Err(box_err!("PD cluster failed to respond")),
    }
}

fn connect(
    env: Arc<Environment>,
    security_mgr: &SecurityManager,
    addr: &str,
) -> Result<(PdClient, GetMembersResponse)> {
    info!("connect to PD endpoint: {:?}", addr);
    let addr = addr
        .trim_left_matches("http://")
        .trim_left_matches("https://");
    let cb = ChannelBuilder::new(env)
        .keepalive_time(Duration::from_secs(10))
        .keepalive_timeout(Duration::from_secs(3));

    let channel = security_mgr.connect(cb, addr);
    let client = PdClient::new(channel);
    let option = CallOption::default().timeout(Duration::from_secs(REQUEST_TIMEOUT));
    match client.get_members_opt(&GetMembersRequest::new(), option) {
        Ok(resp) => Ok((client, resp)),
        Err(e) => Err(Error::Grpc(e)),
    }
}

fn try_connect(
    env: &Arc<Environment>,
    security_mgr: &SecurityManager,
    addr: &str,
    cluster_id: u64,
) -> Result<(PdClient, GetMembersResponse)> {
    let (client, r) = connect(Arc::clone(&env), security_mgr, addr)?;
    let new_cluster_id = r.get_header().get_cluster_id();
    if new_cluster_id != cluster_id {
        Err(box_err!(
            "{} no longer belongs to cluster {}, it is in {}",
            addr,
            cluster_id,
            new_cluster_id
        ))
    } else {
        Ok((client, r))
    }
}

pub fn try_connect_leader(
    env: &Arc<Environment>,
    security_mgr: &SecurityManager,
    previous: &GetMembersResponse,
) -> Result<(PdClient, GetMembersResponse)> {
    let previous_leader = previous.get_leader();
    let members = previous.get_members();
    let cluster_id = previous.get_header().get_cluster_id();
    let mut resp = None;
    // Try to connect to other members, then the previous leader.
    'outer: for m in members
        .into_iter()
        .filter(|m| *m != previous_leader)
        .chain(&[previous_leader.clone()])
    {
        for ep in m.get_client_urls() {
            match try_connect(&env, security_mgr, ep.as_str(), cluster_id) {
                Ok((_, r)) => {
                    resp = Some(r);
                    break 'outer;
                }
                Err(e) => {
                    error!("failed to connect to {}, {:?}", ep, e);
                    continue;
                }
            }
        }
    }

    // Then try to connect the PD cluster leader.
    if let Some(resp) = resp {
        let leader = resp.get_leader().clone();
        for ep in leader.get_client_urls() {
            if let Ok((client, r)) = try_connect(&env, security_mgr, ep.as_str(), cluster_id) {
                return Ok((client, r));
            }
        }
    }

    Err(box_err!("failed to connect to {:?}", members))
}

pub fn check_resp_header(header: &ResponseHeader) -> Result<()> {
    if !header.has_error() {
        Ok(())
    } else {
        Err(box_err!(header.get_error().get_message()))
    }
}

// Re-establish connection with PD leader in synchronized fashion.
pub fn reconnect(leader: &RwLock<LeaderClient>) -> Result<()> {
    println!("try reconnect");
    let ((client, members), start) = {
        let leader = leader.rl();
        if leader.last_update.elapsed() < Duration::from_secs(RECONNECT_INTERVAL_SEC) {
            // Avoid unnecessary updating.
            return Ok(());
        }

        let start = Instant::now();
        (
            try_connect_leader(&leader.env, &leader.security_mgr, &leader.members)?,
            start,
        )
    };

    {
        let mut leader = leader.wl();
        let (tx, rx) = client.tso().unwrap();
        warn!("tso sender and receiver are stale, refreshing..");

        // Try to cancel an unused tso sender.
        if let Either::Left(Some(ref mut r)) = leader.tso_sender {
            info!("cancel tso sender");
            r.cancel();
        }
        leader.tso_sender = Either::Left(Some(tx));
        if let Either::Right(ref mut task) = leader.tso_receiver {
            task.notify();
        }
        leader.tso_receiver = Either::Left(Some(rx));
        leader.client = client;
        leader.members = members;
        leader.last_update = Instant::now();
        if let Some(ref on_reconnect) = leader.on_reconnect {
            on_reconnect();
        }
    }
    warn!("updating PD client done, spent {:?}", start.elapsed());
    Ok(())
}
