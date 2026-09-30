use crate::{
    collector::{self, Collector},
    compact::{self, Catalog, EncodedBatch, Entities},
    model::*,
    store::Store,
    trace::{TraceOptions, Tracer},
};
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use clap::Args;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, Semaphore};

#[derive(Args, Clone, Debug)]
pub struct DaemonArgs {
    #[arg(long, default_value = "127.0.0.1:9387")]
    pub listen: SocketAddr,
    #[arg(long)]
    pub token: Option<String>,
    #[arg(long)]
    pub allow_origin: Vec<String>,
    #[arg(long)]
    pub allow_file_origin: bool,
    #[arg(long)]
    pub allow_unsigned_frontend: bool,
    /// 受信任 Ed25519 发布公钥（32 字节，Base64），可重复
    #[arg(long)]
    pub frontend_public_key: Vec<String>,
    #[arg(long,default_value="1",value_parser=crate::cli::parse_interval)]
    pub interval: f64,
    /// 系统采样组（P1 显式加入 pressure,interrupts,softirq）
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "cpu,memory,load,time,disk,network,vm"
    )]
    pub metrics: Vec<String>,
    #[arg(long,default_value="60",value_parser=clap::value_parser!(u64).range(1..=3600))]
    pub history_seconds: u64,
    #[arg(long,default_value="4194304",value_parser=clap::value_parser!(u64).range(65536..=268435456))]
    pub memory_bytes: u64,
    #[arg(long,default_value="2097152",value_parser=clap::value_parser!(u64).range(65536..=268435456))]
    pub trace_memory_bytes: u64,
    #[arg(long,default_value="1",value_parser=clap::value_parser!(u32).range(1..=32))]
    pub max_connections: u32,
    #[arg(long, default_value = "/proc")]
    pub proc_root: PathBuf,
    #[arg(long, default_value = "self")]
    pub user: String,
    #[arg(long,default_value="500",value_parser=clap::value_parser!(u64).range(1..=60000))]
    pub process_budget_ms: u64,
    #[arg(long)]
    pub prometheus_process: bool,
    #[arg(long)]
    pub timestamp_unix: bool,
    #[arg(long)]
    pub diagnostic_sensitive: bool,
    #[arg(long)]
    pub sqlite_path: Option<PathBuf>,
    #[arg(long, default_value = "16777216")]
    pub sqlite_max_bytes: u64,
    #[arg(long,default_value="3600",value_parser=clap::value_parser!(u64).range(1..))]
    pub sqlite_retention_seconds: u64,
    #[arg(long,default_value="30",value_parser=clap::value_parser!(u64).range(1..=3600))]
    pub sqlite_flush_seconds: u64,
}
struct TraceControl {
    state: String,
    options: Option<TraceOptions>,
    identity: Option<ProcessIdentity>,
    cancel: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    error: Option<String>,
}
impl TraceControl {
    fn json(&self) -> Value {
        json!({"state":self.state,"options":self.options,"identity":self.identity,"error":self.error})
    }
}
struct App {
    args: DaemonArgs,
    token: String,
    uid: u32,
    keys: Vec<VerifyingKey>,
    session: String,
    sequence: AtomicU64,
    // 序号分配、缓存提交和通知顺序必须一致；锁内不做序列化或网络 I/O。
    publish_order: Mutex<()>,
    normal: Mutex<Store>,
    trace_store: Mutex<Store>,
    trace: Mutex<TraceControl>,
    notify: broadcast::Sender<u64>,
    streams: Arc<Semaphore>,
    exports: Arc<Semaphore>,
    running: AtomicBool,
    errors: AtomicU64,
    skipped: AtomicU64,
    sampling: Mutex<[SamplingStats; 3]>,
    transport: Mutex<[TransportStats; 3]>,
    slow_clients: AtomicU64,
    write_timeouts: AtomicU64,
    persistence: crate::persistence::Persistence,
    compact_catalog: Catalog,
    compact_entities: Mutex<Entities>,
}
type Shared = Arc<App>;
fn is_io_timeout(mut error: &(dyn std::error::Error + 'static)) -> bool {
    loop {
        if error
            .downcast_ref::<io::Error>()
            .is_some_and(|e| e.kind() == io::ErrorKind::TimedOut)
        {
            return true;
        }
        match error.source() {
            Some(source) => error = source,
            None => return false,
        }
    }
}
#[derive(Default, Serialize)]
struct TransportStats {
    batches: u64,
    payload_bytes: u64,
}
// 固定三个采样组，每轮更新一次；锁内不读取 procfs、不序列化样本。
#[derive(Default, Serialize)]
struct SamplingStats {
    rounds: u64,
    last_sample_us: Option<u64>,
    max_sample_us: u64,
    budget_us: u64,
    over_budget_rounds: u64,
}
impl SamplingStats {
    fn record(&mut self, elapsed: Duration, budget: Duration) {
        let us = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        self.rounds = self.rounds.saturating_add(1);
        self.last_sample_us = Some(us);
        self.max_sample_us = self.max_sample_us.max(us);
        self.budget_us = budget.as_micros().min(u128::from(u64::MAX)) as u64;
        if elapsed > budget {
            self.over_budget_rounds = self.over_budget_rounds.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    #[test]
    fn sampling_budget_recovers_without_losing_maximum() {
        let mut stats = SamplingStats::default();
        assert_eq!(stats.last_sample_us, None);
        stats.record(Duration::from_micros(1200), Duration::from_micros(1000));
        stats.record(Duration::from_micros(200), Duration::from_micros(1000));
        assert_eq!(stats.rounds, 2);
        assert_eq!(stats.last_sample_us, Some(200));
        assert_eq!(stats.max_sample_us, 1200);
        assert_eq!(stats.over_budget_rounds, 1);
        stats.record(Duration::from_micros(1000), Duration::from_micros(1000));
        assert_eq!(stats.over_budget_rounds, 1);
    }
}
fn error(code: StatusCode, reason: &str) -> Response {
    (code, Json(json!({"error":reason}))).into_response()
}
fn token_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |v, (a, b)| v | (a ^ b)) == 0
}
fn valid_origin(origin: &str) -> bool {
    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };
    matches!(uri.scheme_str(), Some("http" | "https"))
        && uri.authority().is_some()
        && uri.path() == "/"
        && uri.query().is_none()
        && !origin.ends_with('/')
}
async fn guard(State(app): State<Shared>, request: Request<Body>, next: Next) -> Response {
    let origin = request.headers().get("origin").cloned();
    if let Some(value) = &origin {
        let Ok(origin) = value.to_str() else {
            return error(StatusCode::FORBIDDEN, "origin_denied");
        };
        if !(app.args.allow_origin.iter().any(|v| v == origin)
            || (origin == "null" && app.args.allow_file_origin))
        {
            return error(StatusCode::FORBIDDEN, "origin_denied");
        }
    }
    let mut response = if request.method() == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        let auth = request
            .headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "));
        if auth.is_some_and(|s| token_equal(s, &app.token)) {
            next.run(request).await
        } else {
            error(StatusCode::UNAUTHORIZED, "invalid_token")
        }
    };
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if let Some(origin) = origin {
        response
            .headers_mut()
            .insert("access-control-allow-origin", origin);
        response
            .headers_mut()
            .insert("vary", HeaderValue::from_static("Origin"));
        response.headers_mut().insert(
            "access-control-allow-methods",
            HeaderValue::from_static("GET, POST, DELETE, OPTIONS"),
        );
        response.headers_mut().insert(
            "access-control-allow-headers",
            HeaderValue::from_static("Authorization, Content-Type, Last-Event-ID"),
        );
    }
    response
}
impl App {
    fn record_sample(&self, group: usize, elapsed: Duration, budget: Duration) {
        self.sampling.lock().unwrap()[group].record(elapsed, budget);
    }
    fn record_bytes(&self, channel: usize, bytes: usize) {
        // 应用层样本 payload 交给响应流的数量，不代表客户端确认接收。
        let mut stats = self.transport.lock().unwrap();
        stats[channel].batches = stats[channel].batches.saturating_add(1);
        stats[channel].payload_bytes = stats[channel].payload_bytes.saturating_add(bytes as u64);
    }
    fn compact(&self, b: &SampleBatch) -> io::Result<EncodedBatch> {
        let mut entities = self.compact_entities.lock().unwrap();
        compact::encode(b, &self.compact_catalog, &mut entities)
    }
    fn publish(&self, mut b: SampleBatch) {
        for message in &b.diagnostics {
            eprintln!("{message}");
        }
        self.errors
            .fetch_add(b.diagnostics.len() as u64, Ordering::Relaxed);
        if !b.complete {
            self.skipped.fetch_add(1, Ordering::Relaxed);
        }
        b.session_id = self.session.clone();
        let _publish_order = self.publish_order.lock().unwrap();
        b.sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        for sample in &mut b.samples {
            sample.session_id = b.session_id.clone();
            sample.sequence = b.sequence;
        }
        match self.compact(&b) {
            Ok(encoded) => b.wire = Some(Arc::new(encoded)),
            Err(error) => {
                eprintln!("紧凑编码失败: {error}");
                self.errors.fetch_add(1, Ordering::Relaxed);
            }
        }
        let sequence = b.sequence;
        let is_trace = b.group == "trace";
        let b = Arc::new(b);
        self.normal.lock().unwrap().expire(b.uptime_s);
        self.trace_store.lock().unwrap().expire(b.uptime_s);
        self.persistence.offer(b.clone());
        let stored = if is_trace {
            self.trace_store.lock().unwrap().push(b)
        } else {
            self.normal.lock().unwrap().push(b)
        };
        if stored {
            let _ = self.notify.send(sequence);
        } else {
            self.skipped.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn batch(&self, sequence: u64) -> Option<Arc<SampleBatch>> {
        self.normal
            .lock()
            .unwrap()
            .get(sequence)
            .or_else(|| self.trace_store.lock().unwrap().get(sequence))
    }
    fn current(&self) -> Vec<Arc<SampleBatch>> {
        let mut all = self.normal.lock().unwrap().latest();
        all.extend(self.trace_store.lock().unwrap().latest());
        all.sort_by_key(|b| b.sequence);
        all
    }
    fn snapshot(&self, q: &Filter) -> io::Result<Value> {
        let publish_order = self.publish_order.lock().unwrap();
        let mut all: Vec<Arc<SampleBatch>> = self
            .normal
            .lock()
            .unwrap()
            .history
            .iter()
            .map(|(b, _)| b.clone())
            .collect();
        all.extend(
            self.trace_store
                .lock()
                .unwrap()
                .history
                .iter()
                .map(|(b, _)| b.clone()),
        );
        let latest = self.sequence.load(Ordering::Relaxed);
        let lost_through = self
            .normal
            .lock()
            .unwrap()
            .lost_through
            .max(self.trace_store.lock().unwrap().lost_through);
        drop(publish_order);
        all.sort_by_key(|b| b.sequence);
        let oldest = all.first().map(|b| b.sequence);
        let limit = q.limit.unwrap_or(1000);
        let selected: Vec<_> = all
            .into_iter()
            .filter(|b| q.matches(b))
            .take(limit + 1)
            .collect();
        let has_more = selected.len() > limit;
        let selected = selected
            .into_iter()
            .take(limit)
            .map(|b| {
                let filtered = q.filter(&b);
                if let Some(wire) = &filtered.wire {
                    Ok(wire.value())
                } else {
                    self.compact(&filtered).map(|v| v.value())
                }
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(
            json!({"wire_schema":compact::WIRE_SCHEMA,"schema_version":1,"session_id":self.session,
            "sequence":latest,"oldest_sequence":oldest,"history_gap":q.after.is_some_and(|a|a<lost_through),"lost_through_sequence":lost_through,
            "dictionary":{"groups":compact::GROUPS,"statuses":compact::STATUSES,"process_fields":compact::PROCESS_FIELDS,
                "metrics":self.compact_catalog.dictionary()},"batches":selected,"has_more":has_more,"next_after":selected.last().and_then(|b|b[0].as_u64())}),
        )
    }
}
#[derive(Clone, Debug, Default, Deserialize)]
struct Filter {
    #[serde(skip)]
    pid: Option<u32>,
    after: Option<u64>,
    from: Option<f64>,
    to: Option<f64>,
    limit: Option<usize>,
    metric: Option<String>,
    entity: Option<String>,
    group: Option<String>,
    format: Option<String>,
    follow: Option<u8>,
    wire: Option<String>,
}
impl Filter {
    fn validate(&self) -> bool {
        self.limit.is_none_or(|n| (1..=10000).contains(&n))
            && self.from.is_none_or(|v| v.is_finite() && v >= 0.0)
            && self.to.is_none_or(|v| v.is_finite() && v >= 0.0)
            && !self.from.zip(self.to).is_some_and(|(f, t)| f > t)
            && [&self.metric, &self.entity].iter().all(|v| {
                v.as_ref()
                    .is_none_or(|s| s.len() <= 4096 && s.split(',').count() <= 32)
            })
            && self.follow.is_none_or(|n| n <= 1)
            && self
                .group
                .as_deref()
                .is_none_or(|g| matches!(g, "system" | "process" | "trace"))
            && self.wire.as_deref().is_none_or(|w| w == "compact")
    }
    fn matches(&self, b: &SampleBatch) -> bool {
        self.after.is_none_or(|n| b.sequence > n)
            && self
                .pid
                .is_none_or(|pid| b.processes.iter().any(|p| p.identity.pid == pid))
            && self.from.is_none_or(|n| b.uptime_s >= n)
            && self.to.is_none_or(|n| b.uptime_s <= n)
            && self.group.as_ref().is_none_or(|g| g == &b.group)
    }
    fn filter(&self, b: &SampleBatch) -> SampleBatch {
        let mut b = b.clone();
        b.wire = None;
        b.samples.retain(|s| {
            self.metric
                .as_ref()
                .is_none_or(|m| m.split(',').any(|v| v == s.metric))
                && self
                    .entity
                    .as_ref()
                    .is_none_or(|e| e.split(',').any(|v| v == s.entity))
        });
        if let Some(e) = &self.entity {
            b.processes
                .retain(|p| e.split(',').any(|v| v == p.identity.entity()));
        }
        b
    }
}
async fn capabilities(State(app): State<Shared>) -> Json<Value> {
    let root = app.args.proc_root.clone();
    let mut value = tokio::task::spawn_blocking(move || collector::capabilities_document(&root))
        .await
        .unwrap_or(json!({}));
    value["allow_unsigned_frontend"] = json!(app.args.allow_unsigned_frontend);
    value["sensitive_enabled"] = json!(app.args.diagnostic_sensitive);
    value["persistence_enabled"] = json!(app.args.sqlite_path.is_some());
    value["session_id"] = json!(app.session);
    value["wire_schema"] = json!(compact::WIRE_SCHEMA);
    value["schema_version"] = json!(1);
    value["sampling_interval_s"] = json!(app.args.interval);
    value["performance"] = json!({"health":true,"transport_bytes":true,"bounded":true});
    value["dictionary"] = json!({"groups":compact::GROUPS,"statuses":compact::STATUSES,
        "process_fields":compact::PROCESS_FIELDS,"metrics":app.compact_catalog.dictionary()});
    value["recommended_frontend_version"] = json!(env!("CARGO_PKG_VERSION"));
    Json(value)
}
async fn health(State(app): State<Shared>) -> Json<Value> {
    let n = app.normal.lock().unwrap();
    let t = app.trace_store.lock().unwrap();
    let sampling = app.sampling.lock().unwrap();
    let transport = app.transport.lock().unwrap();
    let window = |store: &Store, group: &str| {
        let mut batches = store.history.iter().filter(|(b, _)| b.group == group);
        let first = batches.next().map(|(b, _)| b);
        let last = batches.next_back().map(|(b, _)| b).or(first);
        json!({"from_uptime":first.map(|b|b.uptime_s),"to_uptime":last.map(|b|b.uptime_s),
            "span_seconds":first.zip(last).map(|(a,b)|(b.uptime_s-a.uptime_s).max(0.0)),
            "lost_through_sequence":store.lost_through})
    };
    let windows = json!({"system":window(&n,"system"),"process":window(&n,"process"),"trace":window(&t,"trace")});
    Json(
        json!({"session_id":app.session,"sequence":app.sequence.load(Ordering::Relaxed),"errors":app.errors.load(Ordering::Relaxed),"skipped_rounds":app.skipped.load(Ordering::Relaxed),"memory_bytes":n.bytes,"trace_memory_bytes":t.bytes,"dropped_batches":n.dropped+t.dropped,"trace":app.trace.lock().unwrap().json(),"persistence":*app.persistence.status.lock().unwrap(),"performance":{"windows":windows,"sampling":{"system":sampling[0],"process":sampling[1],"trace":sampling[2]},"transport":{"sse":transport[0],"jsonl":transport[1],"tsv":transport[2]},"slow_clients":app.slow_clients.load(Ordering::Relaxed),"write_timeouts":app.write_timeouts.load(Ordering::Relaxed)}}),
    )
}
async fn current(State(app): State<Shared>, Query(q): Query<Filter>) -> Response {
    if !q.validate() {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    current_compact(State(app)).await
}

async fn current_compact(State(app): State<Shared>) -> Response {
    let batches = app.current();
    let mut encoded = Vec::new();
    for b in batches {
        match app.compact(&b) {
            Ok(v) => encoded.push(v.value()),
            Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "compact_encode_failed"),
        }
    }
    Json(json!({"wire_schema":compact::WIRE_SCHEMA,"schema_version":1,"session_id":app.session,
        "dictionary":{"groups":compact::GROUPS,"statuses":compact::STATUSES,"process_fields":compact::PROCESS_FIELDS,
        "metrics":app.compact_catalog.dictionary()},"batches":encoded,"trace":app.trace.lock().unwrap().json()})).into_response()
}
async fn series(State(app): State<Shared>, Query(q): Query<Filter>) -> Response {
    if !q.validate() {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    match app.snapshot(&q) {
        Ok(value) => Json(value).into_response(),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "compact_encode_failed"),
    }
}

#[derive(Deserialize)]
struct ApiRange {
    min: u32,
    max: u32,
}
#[derive(Deserialize)]
struct Declaration {
    frontend_version: String,
    build_id: String,
    api_compatibility: ApiRange,
    wire_schema: Option<String>,
}
#[derive(Deserialize)]
struct Handshake {
    frontend_version: String,
    build_id: String,
    api_compatibility: ApiRange,
    wire_schema: Option<String>,
    development: Option<bool>,
    payload: Option<String>,
    signature: Option<String>,
}
fn verify_handshake(app: &App, h: &Handshake) -> Result<bool, &'static str> {
    if h.wire_schema.as_deref() != Some(compact::WIRE_SCHEMA) {
        return Err("wire_incompatible");
    }
    if h.api_compatibility.min > 1
        || h.api_compatibility.max < 1
        || h.api_compatibility.min > h.api_compatibility.max
    {
        return Err("api_incompatible");
    }
    if h.development == Some(true) && h.payload.is_none() && h.signature.is_none() {
        return if app.args.allow_unsigned_frontend {
            Ok(true)
        } else {
            Err("unsigned_frontend_disabled")
        };
    }
    let raw = h.payload.as_ref().ok_or("missing_declaration")?;
    let sig = STANDARD
        .decode(h.signature.as_ref().ok_or("missing_signature")?)
        .map_err(|_| "invalid_signature")?;
    let sig = Signature::from_slice(&sig).map_err(|_| "invalid_signature")?;
    if !app
        .keys
        .iter()
        .any(|k| k.verify_strict(raw.as_bytes(), &sig).is_ok())
    {
        return Err("untrusted_signature");
    }
    let d: Declaration = serde_json::from_str(raw).map_err(|_| "invalid_declaration")?;
    if d.frontend_version != h.frontend_version
        || d.build_id != h.build_id
        || d.api_compatibility.min != h.api_compatibility.min
        || d.api_compatibility.max != h.api_compatibility.max
        || d.wire_schema.as_deref() != Some(compact::WIRE_SCHEMA)
    {
        return Err("declaration_mismatch");
    }
    Ok(false)
}
async fn handshake(State(app): State<Shared>, Json(h): Json<Handshake>) -> Response {
    match verify_handshake(&app, &h) {
        Ok(development) => {
            Json(json!({"accepted":true,"development":development,"api_version":1,"wire_schema":compact::WIRE_SCHEMA,"recommended_frontend_version":env!("CARGO_PKG_VERSION")})).into_response()
        }
        Err(reason) => (
            StatusCode::FORBIDDEN,
            Json(json!({"error":reason,"recommended_frontend_version":env!("CARGO_PKG_VERSION")})),
        )
            .into_response(),
    }
}
async fn stream(State(app): State<Shared>, Query(q): Query<Filter>) -> Response {
    if !q.validate() {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    let Ok(permit) = app.streams.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "connection_limit");
    };
    let mut rx = app.notify.subscribe();
    let session = app.session.clone();
    let sequence = app.sequence.load(Ordering::Relaxed);
    let stream = async_stream::stream! {
        let _permit=permit;yield Ok::<Event,Infallible>(Event::default().event("connected").data(json!({"session_id":session,"sequence":sequence,"wire_schema":compact::WIRE_SCHEMA,"schema_version":1,"dictionary":{"groups":compact::GROUPS,"statuses":compact::STATUSES,"process_fields":compact::PROCESS_FIELDS,"metrics":app.compact_catalog.dictionary()}}).to_string()));
        let mut heartbeat=tokio::time::interval(Duration::from_secs(15));heartbeat.tick().await;
        loop{tokio::select!{
            _=heartbeat.tick()=>{yield Ok(Event::default().event("heartbeat").data(json!({"sequence":app.sequence.load(Ordering::Relaxed)}).to_string()));},
            notice=rx.recv()=>{match notice{
                Ok(0)=>{let value=app.trace.lock().unwrap().json();yield Ok(Event::default().event("trace").data(value.to_string()));},
                Ok(sequence)=>{let Some(batch)=app.batch(sequence)else{yield Ok(Event::default().event("error").data("{\"error\":\"history_gap\"}"));break};if q.matches(&batch){let batch=q.filter(&batch);let data=app.compact(&batch).map(|v|json!({"wire_schema":compact::WIRE_SCHEMA,"schema_version":1,"session_id":session,"batch":v.value()}).to_string());match data{Ok(data)=>{app.record_bytes(0, data.len());yield Ok(Event::default().event("sample").id(sequence.to_string()).data(data))},Err(_)=>break}}},
                Err(broadcast::error::RecvError::Closed)=>break,Err(broadcast::error::RecvError::Lagged(_))=>{app.slow_clients.fetch_add(1, Ordering::Relaxed);yield Ok(Event::default().event("error").data("{\"error\":\"slow_client\"}"));break;}
            }}
        }}
    };
    Sse::new(stream).into_response()
}
fn encode_export(app: &App, b: &SampleBatch, format: &str) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    if format != "tsv" {
        let encoded = app
            .compact(b)
            .map_err(|e| io::Error::other(e.to_string()))?;
        serde_json::to_writer(
            &mut out,
            &json!({
                "wire_schema": compact::WIRE_SCHEMA,
                "schema_version": 1,
                "session_id": app.session,
                "dictionary": {"groups":compact::GROUPS,"statuses":compact::STATUSES,"process_fields":compact::PROCESS_FIELDS,"metrics":app.compact_catalog.dictionary()},
                "batch": encoded.value()
            }),
        )?;
        out.push(b'\n');
        return Ok(out);
    }
    for s in &b.samples {
        crate::model::write_tsv(&mut out, s)?;
    }
    Ok(out)
}
async fn export(State(app): State<Shared>, Query(q): Query<Filter>) -> Response {
    if !q.validate() {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    let format = q.format.clone().unwrap_or("jsonl".into());
    if format != "jsonl" && format != "tsv" {
        return error(StatusCode::BAD_REQUEST, "invalid_format");
    }
    let Ok(permit) = app.exports.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "export_limit");
    };
    let mut rx = app.notify.subscribe();
    let mut initial: Vec<_> = app
        .normal
        .lock()
        .unwrap()
        .history
        .iter()
        .map(|(b, _)| b.clone())
        .collect();
    initial.extend(
        app.trace_store
            .lock()
            .unwrap()
            .history
            .iter()
            .map(|(b, _)| b.clone()),
    );
    initial.sort_by_key(|b| b.sequence);
    let last = initial.last().map(|b| b.sequence).unwrap_or(0);
    initial.retain(|b| q.matches(b));
    initial.truncate(q.limit.unwrap_or(1000));
    let content_type = if format == "tsv" {
        "text/tab-separated-values; charset=utf-8"
    } else {
        "application/x-ndjson"
    };
    let stream = async_stream::stream! {let _permit=permit;
        if format=="tsv"{yield Ok::<Bytes,io::Error>(Bytes::from(format!("{}\n",TSV_HEADER)));}
        for batch in initial{match encode_export(&app,&q.filter(&batch),&format){Ok(bytes)=>{app.record_bytes(if format=="tsv" {2} else {1}, bytes.len());yield Ok(Bytes::from(bytes))},Err(e)=>{yield Err(e);return;}}}
        if q.follow==Some(1){loop{match rx.recv().await{Ok(sequence)if sequence>last=>{let Some(batch)=app.batch(sequence)else{yield Err(io::Error::other("history_gap"));break};if q.matches(&batch){match encode_export(&app,&q.filter(&batch),&format){Ok(bytes)=>{app.record_bytes(if format=="tsv" {2} else {1}, bytes.len());yield Ok(Bytes::from(bytes))},Err(e)=>{yield Err(e);break;}}}},Ok(_)=>{},Err(broadcast::error::RecvError::Closed)=>break,Err(broadcast::error::RecvError::Lagged(_))=>{app.slow_clients.fetch_add(1, Ordering::Relaxed);yield Err(io::Error::other("slow_client"));break;}}}}
    };
    (
        [(axum::http::header::CONTENT_TYPE, content_type)],
        Body::from_stream(stream),
    )
        .into_response()
}
async fn processes(State(app): State<Shared>) -> Response {
    let batch = app
        .normal
        .lock()
        .unwrap()
        .latest()
        .into_iter()
        .find(|b| b.group == "process");
    match batch {
        Some(batch) => match app.compact(&batch) {
            Ok(wire) => Json(wire.envelope(&app.session)).into_response(),
            Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "compact_encode_failed"),
        },
        None => Json(json!({"wire_schema":compact::WIRE_SCHEMA,"schema_version":1,"session_id":app.session,"batches":[]})).into_response(),
    }
}

async fn trace_status(State(app): State<Shared>) -> Json<Value> {
    Json(app.trace.lock().unwrap().json())
}
#[derive(Deserialize)]
struct StartTrace {
    pid: u32,
    interval: Option<f64>,
    extended: Option<bool>,
    threads: Option<bool>,
    sensitive: Option<bool>,
    budget_ms: Option<u64>,
}
async fn trace_start(State(app): State<Shared>, Json(request): Json<StartTrace>) -> Response {
    let interval = request.interval.unwrap_or(1.0);
    if !interval.is_finite() || !(1.0..=86400.0).contains(&interval) {
        return error(StatusCode::BAD_REQUEST, "invalid_interval");
    }
    let options = TraceOptions {
        pid: request.pid,
        extended: request.extended.unwrap_or(true),
        threads: request.threads.unwrap_or(false),
        sensitive: request.sensitive.unwrap_or(false),
        budget_ms: request.budget_ms.unwrap_or(500),
    };
    if options.validate().is_err() || options.budget_ms > 60000 {
        return error(StatusCode::BAD_REQUEST, "invalid_trace_options");
    }
    if options.sensitive && !app.args.diagnostic_sensitive {
        return error(StatusCode::FORBIDDEN, "sensitive_disabled");
    }
    let mut state = app.trace.lock().unwrap();
    if matches!(state.state.as_str(), "running" | "stopping")
        || state.handle.as_ref().is_some_and(|h| !h.is_finished())
    {
        return error(StatusCode::CONFLICT, "trace_busy");
    }
    let mut tracer = match Tracer::new(app.args.proc_root.clone(), options.clone(), app.uid) {
        Ok(t) => t,
        Err(_) => return error(StatusCode::BAD_REQUEST, "invalid_trace"),
    };
    tracer.collector.timestamp = app.args.timestamp_unix;
    state.cancel = Arc::new(AtomicBool::new(false));
    state.state = "running".into();
    state.options = Some(options);
    state.identity = None;
    state.error = None;
    let cancel = state.cancel.clone();
    let worker_app = app.clone();
    state.handle = Some(thread::spawn(move || {
        #[cfg(target_os = "linux")]
        unsafe {
            libc::setpriority(libc::PRIO_PROCESS, 0, 10);
        }
        loop {
            if cancel.load(Ordering::Relaxed) || !worker_app.running.load(Ordering::Relaxed) {
                break;
            }
            let deadline = Instant::now() + Duration::from_secs_f64(interval);
            let sample_started = Instant::now();
            let result = tracer.sample();
            worker_app.record_sample(
                2,
                sample_started.elapsed(),
                Duration::from_millis(tracer.options.budget_ms),
            );
            match result {
                Ok(batch) => {
                    worker_app.trace.lock().unwrap().identity = tracer.identity.clone();
                    worker_app.publish(batch);
                }
                Err(e) => {
                    let mut s = worker_app.trace.lock().unwrap();
                    s.state = "exited".into();
                    s.error = Some(e.to_string());
                    eprintln!("trace: {e}");
                    let _ = worker_app.notify.send(0);
                    return;
                }
            }
            while !cancel.load(Ordering::Relaxed)
                && worker_app.running.load(Ordering::Relaxed)
                && Instant::now() < deadline
            {
                thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
            }
        }
        worker_app.trace.lock().unwrap().state = "idle".into();
        let _ = worker_app.notify.send(0);
    }));
    let result = state.json();
    drop(state);
    let _ = app.notify.send(0);
    (StatusCode::ACCEPTED, Json(result)).into_response()
}
async fn trace_stop(State(app): State<Shared>) -> Json<Value> {
    let mut state = app.trace.lock().unwrap();
    if state.state == "running" {
        state.state = "stopping".into();
        state.cancel.store(true, Ordering::Relaxed);
        if let Some(handle) = &state.handle {
            handle.thread().unpark();
        }
    }
    let result = state.json();
    drop(state);
    let _ = app.notify.send(0);
    Json(result)
}
async fn process_capabilities(State(app): State<Shared>, Path(pid): Path<u32>) -> Response {
    let root = app.args.proc_root.clone();
    let uid = app.uid;
    let read = tokio::task::spawn_blocking(move || {
        let mut c = Collector::new(root)?;
        c.uid = uid;
        c.process(pid)
    })
    .await;
    match read{Ok(Ok((p,_)))=>Json(json!({"pid":pid,"starttime_ticks":p.starttime_ticks,"basic":true,"extended":true,"sensitive_compiled":cfg!(feature="diagnostic"),"sensitive_enabled":app.args.diagnostic_sensitive})).into_response(),_=>error(StatusCode::NOT_FOUND,"process_unavailable")}
}
async fn process_current(State(app): State<Shared>, Path(pid): Path<u32>) -> Response {
    let b = app
        .trace_store
        .lock()
        .unwrap()
        .history
        .iter()
        .rev()
        .find(|(b, _)| b.processes.iter().any(|p| p.identity.pid == pid))
        .map(|(b, _)| b.clone());
    match b {
        Some(b) => match app.compact(&b) {
            Ok(wire) => Json(wire.envelope(&app.session)).into_response(),
            Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "compact_encode_failed"),
        },
        None => error(StatusCode::NOT_FOUND, "no_trace_data"),
    }
}
async fn process_series(
    State(app): State<Shared>,
    Path(pid): Path<u32>,
    Query(mut q): Query<Filter>,
) -> Response {
    if !q.validate() {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    q.group = Some("trace".into());
    q.pid = Some(pid);
    series(State(app), Query(q)).await
}
fn prom_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
async fn process_stream(
    State(app): State<Shared>,
    Path(pid): Path<u32>,
    Query(mut q): Query<Filter>,
) -> Response {
    let identity = {
        let trace = app.trace.lock().unwrap();
        if matches!(trace.state.as_str(), "running" | "stopping") {
            trace.identity.clone()
        } else {
            None
        }
    };
    let Some(identity) = identity.filter(|i| i.pid == pid) else {
        return error(StatusCode::NOT_FOUND, "no_active_trace");
    };
    q.group = Some("trace".into());
    q.pid = Some(pid);
    q.entity = Some(identity.entity());
    stream(State(app), Query(q)).await
}
async fn metrics(State(app): State<Shared>) -> Response {
    let batches = app.normal.lock().unwrap().latest();
    let mut out = String::new();
    let mut names = std::collections::HashSet::new();
    for b in batches {
        for s in &b.samples {
            if s.status != Status::Ok
                || (!app.args.prometheus_process && s.entity.starts_with("process:"))
            {
                continue;
            }
            if s.value.as_f64().is_none() {
                continue;
            }
            let mut name = format!("procface_{}", s.metric.replace('.', "_"));
            if s.unit == "percent" && !name.ends_with("_percent") {
                name.push_str("_percent");
            }
            let unit = s.unit.replace("/second", "_per_second");
            let base_unit = s.unit.split('/').next().unwrap_or("");
            if !base_unit.is_empty()
                && !name.contains(base_unit)
                && !(base_unit == "milliseconds" && name.contains("_ms"))
            {
                name.push('_');
                name.push_str(&unit);
            }
            if s.kind == Kind::Counter && !name.ends_with("_total") {
                name.push_str("_total");
            }
            let kind = if s.kind == Kind::Counter {
                "counter"
            } else {
                "gauge"
            };
            if names.insert(name.clone()) {
                out.push_str(&format!(
                    "# HELP {name} {} ({})\n# TYPE {name} {kind}\n",
                    s.metric, s.unit
                ));
            }
            let label = if s.entity.starts_with("cpu") {
                "cpu"
            } else if s.metric.starts_with("disk.") {
                "device"
            } else if s.metric.starts_with("network.") {
                "interface"
            } else {
                "entity"
            };
            if s.entity.starts_with("process:") {
                if let Some(p) = b.processes.iter().find(|p| p.identity.entity() == s.entity) {
                    out.push_str(&format!(
                        "{name}{{pid=\"{}\",comm=\"{}\",starttime_ticks=\"{}\"}} {}\n",
                        p.identity.pid,
                        prom_escape(&p.name),
                        p.identity.starttime_ticks,
                        s.value
                    ));
                }
            } else {
                out.push_str(&format!(
                    "{name}{{{label}=\"{}\"}} {}\n",
                    prom_escape(&s.entity),
                    s.value
                ));
            }
        }
    }
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        out,
    )
        .into_response()
}
fn router(app: Shared) -> Router {
    Router::new()
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/health", get(health))
        .route("/api/v1/current", get(current))
        .route("/api/v1/series", get(series))
        .route("/api/v1/stream", get(stream))
        .route("/api/v1/export", get(export))
        .route("/api/v1/processes", get(processes))
        .route(
            "/api/v1/trace",
            get(trace_status).post(trace_start).delete(trace_stop),
        )
        .route(
            "/api/v1/processes/:pid/capabilities",
            get(process_capabilities),
        )
        .route("/api/v1/processes/:pid/current", get(process_current))
        .route("/api/v1/processes/:pid/series", get(process_series))
        .route("/api/v1/processes/:pid/stream", get(process_stream))
        .route("/api/v1/frontend/handshake", post(handshake))
        .route("/metrics", get(metrics))
        .layer(DefaultBodyLimit::max(16384))
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app)
}
pub fn run(args: DaemonArgs) -> io::Result<()> {
    let groups = crate::cli::selected_groups(&args.metrics)?;
    if args.diagnostic_sensitive && !cfg!(feature = "diagnostic") {
        return Err(io::Error::other("当前构建不含敏感诊断"));
    }
    let persistence = crate::persistence::Persistence::start(
        args.sqlite_path.clone(),
        args.sqlite_max_bytes,
        args.sqlite_retention_seconds,
        args.sqlite_flush_seconds,
        collector::read_bounded(&args.proc_root.join("sys/kernel/random/boot_id"), 128)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(random_id()?),
    )
    .map_err(io::Error::other)?;
    if args.allow_origin.iter().any(|s| !valid_origin(s)) {
        return Err(io::Error::other(
            "--allow-origin 必须为完整 http(s) Origin，不含路径、尾斜线或通配符",
        ));
    }
    let token = match &args.token {
        Some(t) if t.len() >= 16 => t.clone(),
        Some(_) => return Err(io::Error::other("token 至少 16 字节")),
        None => {
            let t = random_id()?;
            eprintln!("ProcFace token: {t}");
            t
        }
    };
    let mut keys = Vec::new();
    let mut declared = args.frontend_public_key.clone();
    if let Some(k) = option_env!("PROCFACE_RELEASE_KEY") {
        declared.push(k.into());
    }
    for k in declared {
        let raw = STANDARD.decode(k).map_err(io::Error::other)?;
        let bytes: [u8; 32] = raw
            .try_into()
            .map_err(|_| io::Error::other("Ed25519 公钥必须为 32 字节"))?;
        keys.push(VerifyingKey::from_bytes(&bytes).map_err(io::Error::other)?);
    }
    if args.allow_unsigned_frontend {
        eprintln!("开发调试已启用：允许未签名前端；token、CORS 和 API 检查仍生效");
    }
    let uid = collector::user_id(&args.user).map_err(io::Error::other)?;
    let (notify, _) = broadcast::channel(16);
    let app = Arc::new(App {
        normal: Mutex::new(Store::new(
            args.memory_bytes as usize,
            args.history_seconds as f64,
        )),
        trace_store: Mutex::new(Store::new(
            args.trace_memory_bytes as usize,
            args.history_seconds as f64,
        )),
        streams: Arc::new(Semaphore::new(args.max_connections as usize)),
        exports: Arc::new(Semaphore::new(2)),
        args: args.clone(),
        token,
        uid,
        keys,
        session: random_id()?,
        sequence: AtomicU64::new(0),
        publish_order: Mutex::new(()),
        notify,
        running: AtomicBool::new(true),
        errors: AtomicU64::new(0),
        skipped: AtomicU64::new(0),
        sampling: Mutex::new(Default::default()),
        transport: Mutex::new(Default::default()),
        slow_clients: AtomicU64::new(0),
        write_timeouts: AtomicU64::new(0),
        persistence,
        compact_catalog: compact::Catalog::builtin()?,
        compact_entities: Mutex::new(compact::Entities::default()),
        trace: Mutex::new(TraceControl {
            state: "idle".into(),
            options: None,
            identity: None,
            cancel: Arc::new(AtomicBool::new(false)),
            handle: None,
            error: None,
        }),
    });
    let mut collector = Collector::new(args.proc_root.clone())?;
    collector.uid = uid;
    collector.timestamp = args.timestamp_unix;
    collector.process_budget = Duration::from_millis(args.process_budget_ms);
    collector.uptime()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(args.listen).await?;
        eprintln!("ProcFace daemon: {}", listener.local_addr()?);
        let sampling_app = app.clone();
        let sampler = thread::spawn(move || {
            while sampling_app.running.load(Ordering::Relaxed) {
                let deadline = Instant::now() + Duration::from_secs_f64(args.interval);
                let sample_started = Instant::now();
                let result = collector.system(&groups);
                sampling_app.record_sample(0, sample_started.elapsed(), Duration::from_secs_f64(args.interval));
                match result {
                    Ok(b) => sampling_app.publish(b),
                    Err(e) => {
                        eprintln!("系统采样失败: {e}");
                        sampling_app.errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
                let sample_started = Instant::now();
                let result = collector.processes();
                sampling_app.record_sample(1, sample_started.elapsed(), Duration::from_millis(args.process_budget_ms));
                match result {
                    Ok(b) => sampling_app.publish(b),
                    Err(e) => {
                        eprintln!("进程采样失败: {e}");
                        sampling_app.errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
                while sampling_app.running.load(Ordering::Relaxed) && Instant::now() < deadline {
                    thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
                }
            }
        });
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let sender = Mutex::new(Some(shutdown_tx));
        let shutdown_app = app.clone();
        let sampler_thread = sampler.thread().clone();
        ctrlc::set_handler(move || {
            shutdown_app.running.store(false, Ordering::Relaxed);
            sampler_thread.unpark();
            if let Some(handle) = &shutdown_app.trace.lock().unwrap().handle {
                handle.thread().unpark();
            }
            if let Some(sender) = sender.lock().unwrap().take() {
                let _ = sender.send(());
            }
        })
        .map_err(io::Error::other)?;
        let routes=router(app.clone());
        let socket_slots=Arc::new(Semaphore::new(64));
        let mut connections=tokio::task::JoinSet::new();
        tokio::pin!(shutdown_rx);
        loop {
            tokio::select! {
                _=&mut shutdown_rx=>break,
                _=connections.join_next(), if !connections.is_empty()=>{},
                accepted=listener.accept()=>{
                    let (socket,_)=accepted?;
                    let Ok(permit)=socket_slots.clone().try_acquire_owned()else{drop(socket);continue};
                    let mut socket=tokio_io_timeout::TimeoutStream::new(socket);
                    // Hyper enforces a 30s request-header timeout below.  The
                    // transport itself must not have an idle read timeout: SSE
                    // and followed exports intentionally keep the request open.
                    socket.set_read_timeout(None);
                    socket.set_write_timeout(Some(Duration::from_secs(5)));
                    let service=hyper_util::service::TowerToHyperService::new(routes.clone());
                    let connection_app=app.clone();
                    connections.spawn(async move {
                        let _permit=permit;
                        let io=hyper_util::rt::TokioIo::new(Box::pin(socket));
                        // Keep a header timeout for clients that never finish an HTTP
                        // request, while leaving the connection read side open for
                        // long-lived SSE/export responses.
                        let mut http=hyper::server::conn::http1::Builder::new();
                        http.max_buf_size(65536)
                            .header_read_timeout(Some(Duration::from_secs(30)))
                            .timer(hyper_util::rt::TokioTimer::new());
                        if let Err(error)=http.serve_connection(io,service).await {
                            if is_io_timeout(&error) {
                                connection_app.write_timeouts.fetch_add(1,Ordering::Relaxed);
                            }
                        }
                    });
                }
            }
        }
        connections.abort_all();
        while connections.join_next().await.is_some(){}
        app.running.store(false, Ordering::Relaxed);
        sampler.thread().unpark();
        let _ = sampler.join();
        let trace_worker = app.trace.lock().unwrap().handle.take();
        if let Some(worker) = trace_worker {
            worker.thread().unpark();
            let _ = worker.join();
        }
        app.persistence.shutdown();
        Ok(())
    })
}
