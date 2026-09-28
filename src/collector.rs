use crate::{model::*, parsers};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const DEFAULT_GROUPS: &[&str] = &["cpu", "memory", "load", "time", "disk", "network", "vm"];
pub const GROUPS: &[&str] = &[
    "cpu", "memory", "load", "time", "disk", "network", "vm", "process",
];
pub const FILE_LIMIT: usize = 4 * 1024 * 1024;
pub fn read_bounded(path: &Path, limit: usize) -> io::Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "procfs 文件超过读取上限",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "procfs 文本编码非法"))
}
pub fn io_status(e: &io::Error) -> Status {
    match e.kind() {
        io::ErrorKind::NotFound => Status::Unsupported,
        io::ErrorKind::PermissionDenied => Status::PermissionDenied,
        _ => Status::ParseError,
    }
}
pub fn current_uid() -> u32 {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}
pub fn user_id(user: &str) -> Result<u32, String> {
    if user == "self" {
        return Ok(current_uid());
    }
    if let Ok(id) = user.parse() {
        return Ok(id);
    }
    let passwd = fs::read_to_string("/etc/passwd").map_err(|e| e.to_string())?;
    passwd
        .lines()
        .find_map(|l| {
            let c: Vec<_> = l.split(':').collect();
            if c.first() == Some(&user) {
                c.get(2)?.parse().ok()
            } else {
                None
            }
        })
        .ok_or_else(|| format!("用户不存在: {user}"))
}
pub fn clock_units() -> (u64, u64) {
    #[cfg(unix)]
    {
        unsafe {
            let hz = libc::sysconf(libc::_SC_CLK_TCK);
            let page = libc::sysconf(libc::_SC_PAGESIZE);
            (
                if hz > 0 { hz as u64 } else { 100 },
                if page > 0 { page as u64 } else { 4096 },
            )
        }
    }
    #[cfg(not(unix))]
    {
        (100, 4096)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub group: String,
    pub source: String,
    pub status: Status,
}
pub fn capabilities(root: &Path) -> Vec<Capability> {
    [
        ("time", "uptime"),
        ("cpu", "stat"),
        ("memory", "meminfo"),
        ("load", "loadavg"),
        ("disk", "diskstats"),
        ("network", "net/dev"),
        ("vm", "vmstat"),
        ("process", "self/stat"),
        ("psi", "pressure/cpu"),
        ("protocols", "net/snmp"),
        ("sockets", "net/sockstat"),
        ("interrupts", "interrupts"),
        ("softirq", "softirqs"),
        ("threads", "self/task"),
        ("smaps", "self/smaps_rollup"),
    ]
    .into_iter()
    .map(|(group, source)| {
        let status = if group == "threads" {
            fs::read_dir(root.join(source))
                .map(|_| Status::Ok)
                .unwrap_or_else(|e| io_status(&e))
        } else {
            read_bounded(&root.join(source), FILE_LIMIT)
                .map(|text| {
                    let bad = match group {
                        "time" => parsers::uptime(&text).is_err(),
                        "process" => parsers::process_stat(&text).is_err(),
                        "cpu" => parsers::stat(&text).1.is_empty(),
                        "load" => parsers::load(&text)
                            .iter()
                            .any(|p| p.status == Status::ParseError),
                        "memory" | "vm" => {
                            parsers::keyed(&text).is_empty()
                                || parsers::keyed(&text).values().any(|v| v.is_err())
                        }
                        "disk" => parsers::disks(&text)
                            .iter()
                            .any(|p| p.status == Status::ParseError),
                        "network" => parsers::network(&text)
                            .iter()
                            .any(|p| p.status == Status::ParseError),
                        _ => false,
                    };
                    if bad {
                        Status::ParseError
                    } else {
                        Status::Ok
                    }
                })
                .unwrap_or_else(|e| io_status(&e))
        };
        Capability {
            group: group.into(),
            source: source.into(),
            status,
        }
    })
    .collect()
}
#[derive(Clone, Debug)]
struct Counter {
    value: u64,
    uptime: f64,
    point: Point,
}
pub struct Collector {
    pub root: PathBuf,
    pub session_id: String,
    pub sequence: u64,
    pub timestamp: bool,
    pub uid: u32,
    pub pids: Vec<u32>,
    pub process_budget: Duration,
    hz: u64,
    page_size: u64,
    previous: HashMap<String, Counter>,
    cpu: HashMap<String, parsers::Cpu>,
    process_cpu: HashMap<String, (u64, f64)>,
    last_system: Option<f64>,
    last_process: Option<f64>,
    last_wall: Option<f64>,
}
impl Collector {
    pub fn new(root: PathBuf) -> io::Result<Self> {
        let (hz, page_size) = clock_units();
        Ok(Self {
            root,
            session_id: random_id()?,
            sequence: 0,
            timestamp: false,
            uid: current_uid(),
            pids: Vec::new(),
            process_budget: Duration::from_millis(500),
            hz,
            page_size,
            previous: HashMap::new(),
            cpu: HashMap::new(),
            process_cpu: HashMap::new(),
            last_system: None,
            last_process: None,
            last_wall: None,
        })
    }
    pub fn uptime(&self) -> io::Result<f64> {
        parsers::uptime(&read_bounded(&self.root.join("uptime"), 1024)?).map_err(io::Error::other)
    }
    fn batch(&mut self, group: &str, uptime: f64) -> SampleBatch {
        self.sequence += 1;
        let wall = if self.timestamp {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs_f64())
        } else {
            None
        };
        let timestamp = wall.filter(|v| self.last_wall.is_none_or(|prev| *v >= prev));
        if wall.is_some() {
            self.last_wall = wall;
        }
        SampleBatch {
            schema_version: 1,
            session_id: self.session_id.clone(),
            sequence: self.sequence,
            uptime_s: uptime,
            timestamp_unix: timestamp,
            group: group.into(),
            samples: Vec::new(),
            processes: Vec::new(),
            complete: true,
            diagnostics: Vec::new(),
        }
    }
    pub(crate) fn stamp(batch: &mut SampleBatch, p: Point) {
        batch.samples.push(Sample {
            schema_version: 1,
            session_id: batch.session_id.clone(),
            sequence: batch.sequence,
            uptime_s: batch.uptime_s,
            timestamp_unix: batch.timestamp_unix,
            metric: p.metric,
            entity: p.entity,
            value: p.value,
            unit: p.unit,
            kind: p.kind,
            status: p.status,
        });
    }
    fn source(
        &self,
        path: &str,
        group: &str,
        parse: fn(&str) -> Vec<Point>,
        batch: &mut SampleBatch,
    ) -> Vec<Point> {
        match read_bounded(&self.root.join(path), FILE_LIMIT) {
            Ok(t) => parse(&t),
            Err(e) => {
                batch.diagnostics.push(format!("{path}: {e}"));
                let status = io_status(&e);
                let mut points = parse("");
                if points.is_empty() {
                    points.push(Point::missing(
                        format!("{group}.available"),
                        "system",
                        "boolean",
                        Kind::Gauge,
                        status,
                    ));
                }
                for p in &mut points {
                    p.value = Value::Null;
                    p.status = status;
                }
                // 文件暂时不可读不等于其中的设备退出，保留已知实体及实际错误。
                for old in self.previous.values() {
                    if old.point.metric.starts_with(&format!("{group}."))
                        && !points
                            .iter()
                            .any(|p| p.metric == old.point.metric && p.entity == old.point.entity)
                    {
                        let mut point = old.point.clone();
                        point.value = Value::Null;
                        point.status = status;
                        points.push(point);
                    }
                }
                points
            }
        }
    }
    pub fn system(&mut self, groups: &[String]) -> io::Result<SampleBatch> {
        let uptime = self.uptime()?;
        let rewind = self.last_system.is_some_and(|v| uptime < v);
        let dt = self.last_system.map(|v| uptime - v);
        let mut batch = self.batch("system", uptime);
        let mut points = Vec::new();
        for group in groups {
            match group.as_str() {
                "time" => {
                    points.push(Point::new(
                        "time.uptime_seconds",
                        "system",
                        uptime,
                        "seconds",
                        Kind::Gauge,
                    ));
                    points.push(match dt.filter(|v| *v > 0.0) {
                        Some(v) => {
                            Point::new("time.interval_seconds", "system", v, "seconds", Kind::Gauge)
                        }
                        None => Point::missing(
                            "time.interval_seconds",
                            "system",
                            "seconds",
                            Kind::Gauge,
                            if rewind {
                                Status::Discontinuity
                            } else {
                                Status::Stale
                            },
                        ),
                    });
                }
                "memory" => {
                    points.extend(self.source("meminfo", "memory", parsers::memory, &mut batch))
                }
                "load" => points.extend(self.source("loadavg", "load", parsers::load, &mut batch)),
                "vm" => points.extend(self.source("vmstat", "vm", parsers::vm, &mut batch)),
                "disk" => {
                    points.extend(self.source("diskstats", "disk", parsers::disks, &mut batch))
                }
                "network" => {
                    points.extend(self.source("net/dev", "network", parsers::network, &mut batch))
                }
                "cpu" => match read_bounded(&self.root.join("stat"), FILE_LIMIT) {
                    Ok(text) => {
                        let (mut p, cpus) = parsers::stat(&text);
                        let mut next = HashMap::new();
                        for cpu in cpus {
                            let before = self.cpu.get(&cpu.entity);
                            let decreased = before.is_some_and(|b| {
                                cpu.ticks.iter().zip(b.ticks).any(|(n, old)| *n < old)
                            });
                            let delta = before.map(|b| {
                                std::array::from_fn::<_, 8, _>(|i| {
                                    cpu.ticks[i].saturating_sub(b.ticks[i])
                                })
                            });
                            let total = delta
                                .map(|d| d.iter().map(|x| *x as f64).sum::<f64>())
                                .unwrap_or(0.0);
                            for (name, idx) in [
                                ("usage", None),
                                ("user", Some(0)),
                                ("nice", Some(1)),
                                ("system", Some(2)),
                                ("idle", Some(3)),
                                ("iowait", Some(4)),
                                ("irq", Some(5)),
                                ("softirq", Some(6)),
                                ("steal", Some(7)),
                            ] {
                                let status = if rewind || decreased {
                                    Status::Discontinuity
                                } else if before.is_none()
                                    || total == 0.0
                                    || dt.is_none_or(|v| v <= 0.0)
                                {
                                    Status::Stale
                                } else {
                                    Status::Ok
                                };
                                let point = if status == Status::Ok {
                                    let d = delta.unwrap();
                                    let n = idx.map(|i| d[i] as f64).unwrap_or(total - d[3] as f64);
                                    Point::new(
                                        format!("cpu.{name}"),
                                        &cpu.entity,
                                        100.0 * n / total,
                                        "percent",
                                        Kind::Gauge,
                                    )
                                } else {
                                    Point::missing(
                                        format!("cpu.{name}"),
                                        &cpu.entity,
                                        "percent",
                                        Kind::Gauge,
                                        status,
                                    )
                                };
                                p.push(point);
                            }
                            next.insert(cpu.entity.clone(), cpu);
                        }
                        self.cpu = next;
                        points.extend(p);
                    }
                    Err(e) => {
                        batch.diagnostics.push(format!("stat: {e}"));
                        points.extend(parsers::stat("").0.into_iter().map(|mut point| {
                            point.status = io_status(&e);
                            point
                        }));
                        self.cpu.clear();
                    }
                },
                "process" => {}
                _ => return Err(io::Error::other(format!("未知指标组: {group}"))),
            }
        }
        let mut seen = HashSet::new();
        let old_keys: Vec<_> = self.previous.keys().cloned().collect();
        for mut point in points {
            let key = format!("{}\0{}", point.metric, point.entity);
            seen.insert(key.clone());
            if point.kind == Kind::Counter {
                let raw = point.value.as_u64();
                let previous = self.previous.get(&key);
                let source_status = point.status;
                let status = if source_status != Status::Ok {
                    source_status
                } else if rewind || previous.zip(raw).is_some_and(|(old, n)| n < old.value) {
                    Status::Discontinuity
                } else if previous.is_none() || dt.is_none_or(|v| v <= 0.0) {
                    Status::Stale
                } else {
                    Status::Ok
                };
                if let Some(name) = &point.rate_metric {
                    let unit = format!("{}/second", point.unit);
                    let rate = if status == Status::Ok {
                        let old = previous.unwrap();
                        Point::new(
                            name,
                            &point.entity,
                            (raw.unwrap() - old.value) as f64 / (uptime - old.uptime),
                            &unit,
                            Kind::Rate,
                        )
                    } else {
                        Point::missing(name, &point.entity, &unit, Kind::Rate, status)
                    };
                    Self::stamp(&mut batch, rate);
                }
                if let Some(value) = raw {
                    self.previous.insert(
                        key,
                        Counter {
                            value,
                            uptime,
                            point: point.clone(),
                        },
                    );
                } else {
                    self.previous.remove(&key);
                }
                if status == Status::Discontinuity {
                    point.value = Value::Null;
                    point.status = status;
                }
            }
            Self::stamp(&mut batch, point);
        }
        for key in old_keys {
            if !seen.contains(&key) {
                if let Some(old) = self.previous.remove(&key) {
                    let mut p = old.point;
                    p.value = Value::Null;
                    p.status = Status::Exited;
                    Self::stamp(&mut batch, p.clone());
                    if let Some(name) = p.rate_metric {
                        Self::stamp(
                            &mut batch,
                            Point::missing(
                                name,
                                p.entity,
                                &format!("{}/second", p.unit),
                                Kind::Rate,
                                Status::Exited,
                            ),
                        );
                    }
                }
            }
        }
        self.last_system = Some(uptime);
        Ok(batch)
    }
    pub fn process(&self, pid: u32) -> io::Result<(parsers::ProcessStat, u32)> {
        let dir = self.root.join(pid.to_string());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if fs::metadata(&dir)?.uid() != self.uid {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "不在选定用户范围内",
                ));
            }
        }
        let status = read_bounded(&dir.join("status"), FILE_LIMIT)?;
        let uid = status
            .lines()
            .find_map(|l| l.strip_prefix("Uid:"))
            .and_then(|l| l.split_whitespace().next())
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "进程 UID 缺失或非法"))?;
        if uid != self.uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "不在选定用户范围内",
            ));
        }
        let first = parsers::process_stat(&read_bounded(&dir.join("stat"), FILE_LIMIT)?)
            .map_err(io::Error::other)?;
        if first.pid != pid {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "进程 PID 不一致",
            ));
        }
        Ok((first, uid))
    }
    pub fn processes(&mut self) -> io::Result<SampleBatch> {
        let uptime = self.uptime()?;
        let start = Instant::now();
        let rewind = self.last_process.is_some_and(|v| uptime < v);
        let mut batch = self.batch("process", uptime);
        let pids = if self.pids.is_empty() {
            fs::read_dir(&self.root)?
                .filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
                .collect::<Vec<_>>()
        } else {
            self.pids.clone()
        };
        let mut next = HashMap::new();
        for pid in pids {
            if start.elapsed() >= self.process_budget {
                batch.complete = false;
                batch.samples.clear();
                batch.processes.clear();
                batch.diagnostics.push("进程扫描超时，跳过本轮".into());
                return Ok(batch);
            }
            #[cfg(unix)]
            if self.pids.is_empty() {
                use std::os::unix::fs::MetadataExt;
                if std::fs::metadata(self.root.join(pid.to_string()))
                    .is_ok_and(|m| m.uid() != self.uid)
                {
                    continue;
                }
            }
            if start.elapsed() >= self.process_budget {
                batch.complete = false;
                batch.samples.clear();
                batch.processes.clear();
                batch.diagnostics.push("进程扫描超时，跳过本轮".into());
                return Ok(batch);
            }
            match self.process(pid) {
                Ok((s, uid)) => {
                    let identity = ProcessIdentity {
                        pid,
                        starttime_ticks: s.starttime_ticks,
                    };
                    let entity = identity.entity();
                    let ticks = s.user_ticks.saturating_add(s.system_ticks);
                    let old = self.process_cpu.get(&entity);
                    let status = if rewind || old.is_some_and(|(t, _)| ticks < *t) {
                        Status::Discontinuity
                    } else if old.is_some_and(|(_, up)| uptime > *up) {
                        Status::Ok
                    } else {
                        Status::Stale
                    };
                    let percent = old.filter(|_| status == Status::Ok).map(|(t, up)| {
                        100.0 * (ticks - *t) as f64 / self.hz as f64 / (uptime - *up)
                    });
                    let rss = s
                        .rss_pages
                        .checked_mul(self.page_size)
                        .ok_or_else(|| io::Error::other("RSS 溢出"))?;
                    for p in [
                        Point::new("process.pid", &entity, pid, "id", Kind::Gauge),
                        Point::new("process.name", &entity, s.name.clone(), "text", Kind::Gauge),
                        Point::new(
                            "process.state",
                            &entity,
                            s.state.clone(),
                            "text",
                            Kind::Gauge,
                        ),
                        Point::new(
                            "process.cpu_seconds",
                            &entity,
                            ticks as f64 / self.hz as f64,
                            "seconds",
                            Kind::Counter,
                        ),
                        Point::new("process.rss_bytes", &entity, rss, "bytes", Kind::Gauge),
                        Point::new(
                            "process.virtual_bytes",
                            &entity,
                            s.virtual_bytes,
                            "bytes",
                            Kind::Gauge,
                        ),
                        Point::new(
                            "process.threads",
                            &entity,
                            s.threads,
                            "threads",
                            Kind::Gauge,
                        ),
                    ] {
                        Self::stamp(&mut batch, p);
                    }
                    Self::stamp(
                        &mut batch,
                        match percent {
                            Some(v) => {
                                Point::new("process.cpu_usage", &entity, v, "percent", Kind::Gauge)
                            }
                            None => Point::missing(
                                "process.cpu_usage",
                                &entity,
                                "percent",
                                Kind::Gauge,
                                status,
                            ),
                        },
                    );
                    batch.processes.push(ProcessInfo {
                        identity,
                        name: s.name,
                        state: s.state,
                        uid,
                        rss_bytes: rss,
                        threads: s.threads,
                        cpu_seconds: ticks as f64 / self.hz as f64,
                        cpu_percent: percent,
                    });
                    next.insert(entity, (ticks, uptime));
                }
                Err(e) => batch.diagnostics.push(format!("PID {pid}: {e}")),
            }
        }
        if start.elapsed() >= self.process_budget {
            batch.complete = false;
            batch.samples.clear();
            batch.processes.clear();
            batch.diagnostics.push("进程扫描超时，跳过本轮".into());
            return Ok(batch);
        }
        for entity in self.process_cpu.keys() {
            if !next.contains_key(entity) {
                Self::stamp(
                    &mut batch,
                    Point::missing(
                        "process.cpu_usage",
                        entity,
                        "percent",
                        Kind::Gauge,
                        Status::Exited,
                    ),
                );
            }
        }
        self.process_cpu = next;
        self.last_process = Some(uptime);
        Ok(batch)
    }
}
pub fn capabilities_document(root: &Path) -> Value {
    json!({"procface_version":env!("CARGO_PKG_VERSION"),"api_version":API_VERSION,"schema_version":SCHEMA_VERSION,"api_compatibility":{"min":1,"max":1},"recommended_frontend_version":env!("CARGO_PKG_VERSION"),"features":{"sqlite":cfg!(feature="sqlite"),"sensitive":cfg!(feature="diagnostic")},"groups":capabilities(root)})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!("procface-{}", random_id().unwrap()));
        fs::create_dir_all(p.join("net")).unwrap();
        p
    }
    #[test]
    fn rates_rewind_disappear_and_missing() {
        let p = fixture();
        fs::write(p.join("uptime"), "10 0").unwrap();
        fs::write(
            p.join("net/dev"),
            "eth0: 100 1 0 0 0 0 0 0 200 1 0 0 0 0 0 0",
        )
        .unwrap();
        let mut c = Collector::new(p.clone()).unwrap();
        let groups = vec!["network".into(), "memory".into()];
        let a = c.system(&groups).unwrap();
        assert!(a
            .samples
            .iter()
            .any(|s| s.metric == "memory.total_bytes" && s.status == Status::Unsupported));
        fs::write(p.join("uptime"), "12 0").unwrap();
        fs::write(
            p.join("net/dev"),
            "eth0: 160 1 0 0 0 0 0 0 220 1 0 0 0 0 0 0",
        )
        .unwrap();
        let b = c.system(&groups).unwrap();
        assert_eq!(
            b.samples
                .iter()
                .find(|s| s.metric == "network.rx_bytes_per_second")
                .unwrap()
                .value
                .as_f64(),
            Some(30.0)
        );
        fs::write(p.join("uptime"), "1 0").unwrap();
        let b = c.system(&groups).unwrap();
        assert_eq!(
            b.samples
                .iter()
                .find(|s| s.metric == "network.rx_bytes_per_second")
                .unwrap()
                .status,
            Status::Discontinuity
        );
        fs::write(p.join("uptime"), "2 0").unwrap();
        fs::write(p.join("net/dev"), "").unwrap();
        let b = c.system(&groups).unwrap();
        assert!(b.samples.iter().any(|s| s.status == Status::Exited));
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn cpu_deltas_and_guest_not_double_counted() {
        let p = fixture();
        fs::write(p.join("uptime"), "1 0").unwrap();
        fs::write(p.join("stat"), "cpu 10 0 10 80 0 0 0 0 10 0").unwrap();
        let mut c = Collector::new(p.clone()).unwrap();
        let g = vec!["cpu".into()];
        c.system(&g).unwrap();
        fs::write(p.join("uptime"), "2 0").unwrap();
        fs::write(p.join("stat"), "cpu 20 0 20 160 0 0 0 0 20 0").unwrap();
        let b = c.system(&g).unwrap();
        assert_eq!(
            b.samples
                .iter()
                .find(|s| s.metric == "cpu.usage")
                .unwrap()
                .value
                .as_f64(),
            Some(20.0)
        );
        fs::write(p.join("uptime"), "3 0").unwrap();
        fs::write(p.join("stat"), "cpu 1 0 1 8 0 0 0 0").unwrap();
        let b = c.system(&g).unwrap();
        assert!(b.samples.iter().any(|s| s.metric == "cpu.usage"
            && s.status == Status::Discontinuity
            && s.value.is_null()));
        fs::write(p.join("uptime"), "4 0").unwrap();
        fs::write(p.join("stat"), "cpu 2 0 2 16 0 0 0 0").unwrap();
        assert_eq!(
            c.system(&g)
                .unwrap()
                .samples
                .iter()
                .find(|s| s.metric == "cpu.usage")
                .unwrap()
                .value
                .as_f64(),
            Some(20.0)
        );
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn disk_reset_source_error_and_reappearance() {
        let p = fixture();
        fs::write(p.join("uptime"), "10 0").unwrap();
        fs::write(p.join("diskstats"), "8 0 sda 1 0 4 6 2 0 8 9 0 10 11").unwrap();
        let mut c = Collector::new(p.clone()).unwrap();
        let groups = vec!["disk".into()];
        c.system(&groups).unwrap();
        fs::write(p.join("uptime"), "12 0").unwrap();
        fs::write(p.join("diskstats"), "8 0 sda 2 0 8 7 2 0 8 9 0 10 11").unwrap();
        let b = c.system(&groups).unwrap();
        let rate = b
            .samples
            .iter()
            .find(|s| s.metric == "disk.read_bytes_per_second")
            .unwrap();
        assert_eq!(rate.value.as_f64(), Some(1024.0));
        assert_eq!(rate.unit, "bytes/second");
        fs::write(p.join("uptime"), "13 0").unwrap();
        fs::write(p.join("diskstats"), "8 0 sda 0 0 0 0 0 0 0 0 0 0 0").unwrap();
        let b = c.system(&groups).unwrap();
        assert!(
            b.samples
                .iter()
                .any(|s| s.metric == "disk.read_bytes_per_second"
                    && s.status == Status::Discontinuity)
        );
        fs::remove_file(p.join("diskstats")).unwrap();
        let b = c.system(&groups).unwrap();
        assert!(b
            .samples
            .iter()
            .any(|s| s.entity == "sda" && s.status == Status::Unsupported));
        assert!(!b.samples.iter().any(|s| s.status == Status::Exited));
        fs::write(p.join("uptime"), "14 0").unwrap();
        fs::write(p.join("diskstats"), "8 0 sda 3 0 10 8 2 0 8 9 0 10 11").unwrap();
        let b = c.system(&groups).unwrap();
        assert!(b
            .samples
            .iter()
            .any(|s| s.metric == "disk.read_bytes_per_second" && s.status == Status::Stale));
        fs::write(p.join("diskstats"), "").unwrap();
        let b = c.system(&groups).unwrap();
        assert!(b
            .samples
            .iter()
            .any(|s| s.entity == "sda" && s.status == Status::Exited));
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn missing_and_invalid_groups_do_not_hide_valid_samples() {
        let p = fixture();
        fs::write(p.join("uptime"), "10 0").unwrap();
        fs::write(p.join("meminfo"), "MemTotal: 1024 kB\nMemFree: bad kB").unwrap();
        fs::write(p.join("vmstat"), "pgpgin 2\npgpgout bad\npgfault 7").unwrap();
        fs::write(p.join("loadavg"), "0.1 0.2 0.3 2/9 42").unwrap();
        let mut c = Collector::new(p.clone()).unwrap();
        let groups = vec!["time".into(), "memory".into(), "vm".into(), "load".into()];
        let b = c.system(&groups).unwrap();
        for (metric, value) in [
            ("memory.total_bytes", 1048576),
            ("vm.page_in_bytes", 2048),
            ("load.running", 2),
        ] {
            assert_eq!(
                b.samples
                    .iter()
                    .find(|s| s.metric == metric)
                    .unwrap()
                    .value
                    .as_u64(),
                Some(value)
            );
        }
        for (metric, status) in [
            ("memory.free_bytes", Status::ParseError),
            ("vm.page_out_bytes", Status::ParseError),
            ("memory.available_bytes", Status::Unsupported),
            ("vm.major_faults", Status::Unsupported),
        ] {
            let s = b.samples.iter().find(|s| s.metric == metric).unwrap();
            assert_eq!(s.status, status);
            assert!(s.value.is_null());
        }
        let cap = capabilities(&p);
        assert_eq!(
            cap.iter().find(|g| g.group == "memory").unwrap().status,
            Status::ParseError
        );
        assert_eq!(
            cap.iter().find(|g| g.group == "cpu").unwrap().status,
            Status::Unsupported
        );
        fs::write(p.join("uptime"), "12 0").unwrap();
        let b = c.system(&groups).unwrap();
        assert_eq!(
            b.samples
                .iter()
                .find(|s| s.metric == "time.interval_seconds")
                .unwrap()
                .value
                .as_f64(),
            Some(2.0)
        );
        fs::write(p.join("uptime"), "1 0").unwrap();
        assert!(c
            .system(&groups)
            .unwrap()
            .samples
            .iter()
            .any(|s| s.metric == "time.interval_seconds" && s.status == Status::Discontinuity));
        #[cfg(unix)]
        if current_uid() != 0 {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(p.join("meminfo"), fs::Permissions::from_mode(0o0)).unwrap();
            assert_eq!(
                capabilities(&p)
                    .iter()
                    .find(|g| g.group == "memory")
                    .unwrap()
                    .status,
                Status::PermissionDenied
            );
            let b = c.system(&groups).unwrap();
            assert!(b
                .samples
                .iter()
                .any(|s| s.metric == "memory.total_bytes" && s.status == Status::PermissionDenied));
            assert!(b
                .samples
                .iter()
                .any(|s| s.metric == "load.1m" && s.status == Status::Ok));
        }
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn bounded_read() {
        let p = fixture();
        fs::write(p.join("large"), "12345").unwrap();
        assert!(read_bounded(&p.join("large"), 4).is_err());
        fs::remove_dir_all(p).unwrap();
    }
}
