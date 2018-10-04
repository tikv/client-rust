use fxhash::FxHashSet as HashSet;
use std::result;
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::time::Instant;

use futures::future::{loop_fn, ok, Loop};
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::sync::oneshot;
use futures::{Future, Sink, Stream};
use grpc::{CallOption, ChannelBuilder, Environment, WriteFlags};
use kvproto::pdpb::{
    GetMembersRequest, GetMembersResponse, ResponseHeader, TsoRequest, TsoResponse,
};
use kvproto::pdpb_grpc::PdClient;
use tokio_timer::timer::Handle;

use super::{Error, PdFuture, PdTimestamp, Result, PD_REQUEST_HISTOGRAM_VEC, REQUEST_TIMEOUT};
use tokio_core::reactor::{Core, Handle as OtherHandle};
use util::security::SecurityManager;
use util::time::duration_to_sec;
use util::timer::GLOBAL_TIMER_HANDLE;
use util::HandyRwLock;

macro_rules! request {
    ($cluster_id:expr, $type:ty) => {{
        let mut request = <$type>::new();
        let mut header = ::kvproto::pdpb::RequestHeader::new();
        header.set_cluster_id($cluster_id);
        request.set_header(header);
        request
    }};
}

type TsoChannel = oneshot::Sender<PdTimestamp>;
type PdRequest = Box<Future<Item = (), Error = ()> + Send>;

pub enum PdTask {
    TsoInit,
    TsoRequest,
    TsoResponse(Vec<oneshot::Sender<PdTimestamp>>, TsoResponse),
    Request(PdRequest),
}

struct PdReactor {
    task_tx: Option<UnboundedSender<Option<PdTask>>>,
    tso_tx: UnboundedSender<TsoRequest>,
    tso_rx: Option<UnboundedReceiver<TsoRequest>>,

    handle: Option<JoinHandle<()>>,
    tso_pending: Option<Vec<TsoChannel>>,
    tso_buffer: Option<Vec<TsoChannel>>,
    tso_batch: Vec<TsoChannel>,
}

impl Drop for PdReactor {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}

impl PdReactor {
    fn new() -> PdReactor {
        let (tso_tx, tso_rx) = unbounded();
        PdReactor {
            task_tx: None,
            tso_tx,
            tso_rx: Some(tso_rx),
            handle: None,
            tso_buffer: Some(Vec::with_capacity(8)),
            tso_batch: Vec::with_capacity(8),
            tso_pending: None,
        }
    }

    fn start(&mut self, client: Arc<RwLock<LeaderClient>>) {
        if self.handle.is_none() {
            info!("starting pd reactor thread");
            let (task_tx, task_rx) = unbounded();
            task_tx.unbounded_send(Some(PdTask::TsoInit)).unwrap();
            self.task_tx = Some(task_tx);
            self.handle = Some(
                thread::Builder::new()
                    .name("dispatcher thread".to_owned())
                    .spawn(move || Self::poll(&client, task_rx))
                    .unwrap(),
            )
        } else {
            warn!("tso sender and receiver are stale, refreshing..");
            let (tso_tx, tso_rx) = unbounded();
            self.tso_tx = tso_tx;
            self.tso_rx = Some(tso_rx);
            self.schedule(PdTask::TsoInit);
        }
    }

    fn schedule(&self, task: PdTask) {
        self.task_tx
            .as_ref()
            .unwrap()
            .unbounded_send(Some(task))
            .unwrap();
    }

    fn poll(client: &Arc<RwLock<LeaderClient>>, rx: UnboundedReceiver<Option<PdTask>>) {
        let mut core = Core::new().unwrap();
        let handle = core.handle();
        {
            let f = rx.take_while(|t| Ok(t.is_some())).for_each(|t| {
                Self::dispatch(&client, t.unwrap(), &handle);
                Ok(())
            });
            core.run(f).unwrap();
        }
    }

    fn init(client: &Arc<RwLock<LeaderClient>>, handle: &OtherHandle) {
        let client = Arc::clone(client);
        let (tx, rx) = client.wl().client.tso().unwrap();
        let tso_rx = client.wl().reactor.tso_rx.take().unwrap();
        handle.spawn(
            tx.sink_map_err(Error::Grpc)
                .send_all(tso_rx.then(|r| match r {
                    Ok(r) => Ok((r, WriteFlags::default())),
                    Err(()) => Err(Error::Other(box_err!("failed to recv tso requests"))),
                })).then(|r| match r {
                    Ok((mut sender, _)) => {
                        sender.get_mut().cancel();
                        Ok(())
                    }
                    Err(e) => {
                        error!("failed to send tso requests: {:?}", e);
                        Err(())
                    }
                }),
        );
        handle.spawn(
            rx.for_each(move |resp| {
                let mut client = client.wl();
                let reactor = &mut client.reactor;
                let tso_pending = reactor.tso_pending.take().unwrap();
                reactor.schedule(PdTask::TsoResponse(tso_pending, resp));
                if !reactor.tso_batch.is_empty() {
                    /* schedule another tso_batch of request */
                    reactor.schedule(PdTask::TsoRequest);
                }
                Ok(())
            }).map_err(|e| panic!("unexpected error: {:?}", e)),
        );
    }

    fn tso_request(client: &Arc<RwLock<LeaderClient>>) {
        let mut client = client.wl();
        let cluster_id = client.cluster_id;
        let reactor = &mut client.reactor;
        let mut tso_batch = reactor.tso_buffer.take().unwrap();
        tso_batch.extend(reactor.tso_batch.drain(..));
        let mut request = request!(cluster_id, TsoRequest);
        request.set_count(tso_batch.len() as u32);
        reactor.tso_pending = Some(tso_batch);
        reactor.tso_tx.unbounded_send(request).unwrap();
    }

    fn tso_response(
        client: &Arc<RwLock<LeaderClient>>,
        mut requests: Vec<TsoChannel>,
        response: &TsoResponse,
    ) {
        let timestamp = response.get_timestamp();
        for (offset, request) in requests.drain(..).enumerate() {
            request
                .send(PdTimestamp {
                    physical: timestamp.physical,
                    logical: timestamp.logical + offset as i64,
                }).unwrap();
        }
        client.wl().reactor.tso_buffer = Some(requests);
    }

    fn dispatch(client: &Arc<RwLock<LeaderClient>>, task: PdTask, handle: &OtherHandle) {
        match task {
            PdTask::TsoRequest => Self::tso_request(client),
            PdTask::TsoResponse(requests, response) => {
                Self::tso_response(client, requests, &response)
            }
            PdTask::TsoInit => Self::init(client, handle),
            PdTask::Request(task) => handle.spawn(task),
        }
    }

    fn get_ts(&mut self) -> impl Future<Item = PdTimestamp, Error = Error> {
        let timer = Instant::now();
        let (tx, rx) = oneshot::channel::<PdTimestamp>();
        self.tso_batch.push(tx);
        if self.tso_pending.is_none() {
            /* schedule tso request to run */
            self.schedule(PdTask::TsoRequest);
        }
        rx.map_err(Error::Canceled).and_then(move |ts| {
            PD_REQUEST_HISTOGRAM_VEC
                .with_label_values(&["get_ts"])
                .observe(duration_to_sec(timer.elapsed()));
            Ok(ts)
        })
    }
}

pub struct LeaderClient {
    pub client: PdClient,
    pub members: GetMembersResponse,
    pub on_reconnect: Option<Box<Fn() + Sync + Send + 'static>>,

    env: Arc<Environment>,
    cluster_id: u64,
    security_mgr: Arc<SecurityManager>,
    last_update: Instant,
    reactor: PdReactor,
}

impl LeaderClient {
    pub fn new(
        env: Arc<Environment>,
        security_mgr: Arc<SecurityManager>,
        client: PdClient,
        members: GetMembersResponse,
    ) -> Arc<RwLock<LeaderClient>> {
        let cluster_id = members.get_header().get_cluster_id();
        let client = Arc::new(RwLock::new(LeaderClient {
            env,
            client,
            members,
            security_mgr,
            on_reconnect: None,
            last_update: Instant::now(),
            reactor: PdReactor::new(),
            cluster_id,
        }));

        client.wl().reactor.start(Arc::clone(&client));
        client
    }

    pub fn get_ts(&mut self) -> impl Future<Item = PdTimestamp, Error = Error> {
        self.reactor.get_ts()
    }

    pub fn schedule(&self, task: PdRequest) {
        self.reactor.schedule(PdTask::Request(task));
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
pub fn reconnect(leader: &Arc<RwLock<LeaderClient>>) -> Result<()> {
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
        let leader_clone = Arc::clone(leader);
        let mut leader = leader.wl();
        leader.client = client;
        leader.members = members;
        leader.last_update = Instant::now();
        if let Some(ref on_reconnect) = leader.on_reconnect {
            on_reconnect();
        }
        leader.reactor.start(leader_clone);
    }
    warn!("updating PD client done, spent {:?}", start.elapsed());
    Ok(())
}
