use crate::{
    collector::{io_status, read_bounded, Collector, FILE_LIMIT},
    model::*,
    parsers,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs, io,
    path::PathBuf,
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceOptions {
    pub pid: u32,
    pub extended: bool,
    pub threads: bool,
    pub sensitive: bool,
    pub budget_ms: u64,
}
impl TraceOptions {
    pub fn validate(&self) -> Result<(), String> {
        if self.pid == 0 {
            return Err("PID 必须大于零".into());
        }
        if self.budget_ms == 0 {
            return Err("trace 预算必须大于零".into());
        }
        if self.sensitive && !cfg!(feature = "diagnostic") {
            return Err("当前构建不含敏感诊断能力".into());
        }
        Ok(())
    }
}
pub struct Tracer {
    pub collector: Collector,
    pub options: TraceOptions,
    pub identity: Option<ProcessIdentity>,
    io_baseline: HashMap<String, (u64, f64)>,
}
impl Tracer {
    pub fn new(root: PathBuf, options: TraceOptions, uid: u32) -> io::Result<Self> {
        options.validate().map_err(io::Error::other)?;
        let mut collector = Collector::new(root)?;
        collector.uid = uid;
        collector.pids = vec![options.pid];
        collector.process_budget = Duration::from_millis(options.budget_ms);
        Ok(Self {
            collector,
            options,
            identity: None,
            io_baseline: HashMap::new(),
        })
    }
    pub fn sample(&mut self) -> io::Result<SampleBatch> {
        let start = Instant::now();
        let budget = Duration::from_millis(self.options.budget_ms);
        let mut b = self.collector.processes()?;
        b.group = "trace".into();
        if !b.complete {
            return Ok(b);
        }
        let Some(info) = b.processes.first() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                b.diagnostics.join("; "),
            ));
        };
        let identity = info.identity.clone();
        if self.identity.as_ref().is_some_and(|old| old != &identity) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "PID 已复用，trace 已结束",
            ));
        }
        self.identity = Some(identity.clone());
        let entity = identity.entity();
        let dir = self.collector.root.join(self.options.pid.to_string());
        let mut files = vec!["status", "statm", "io", "cgroup"];
        if self.options.extended {
            files.extend([
                "sched",
                "schedstat",
                "smaps_rollup",
                "limits",
                "loginuid",
                "oom_score",
                "oom_score_adj",
            ]);
        }
        #[cfg(feature = "diagnostic")]
        if self.options.sensitive {
            files.extend([
                "cmdline",
                "environ",
                "maps",
                "smaps",
                "numa_maps",
                "wchan",
                "stack",
                "syscall",
                "mountinfo",
            ]);
        }
        let mut bytes = 0usize;
        for file in files {
            if start.elapsed() >= budget || bytes >= FILE_LIMIT {
                b.diagnostics
                    .push(format!("{file}: skipped_expensive（预算耗尽）"));
                continue;
            }
            let limit = (FILE_LIMIT - bytes).min(256 * 1024);
            let point = match read_bounded(&dir.join(file), limit) {
                Ok(text) => {
                    bytes += text.len();
                    if file == "io" {
                        for (key, value) in parsers::keyed(&text) {
                            let unit = if key.ends_with("bytes") || key == "rchar" || key == "wchar"
                            {
                                "bytes"
                            } else {
                                "operations"
                            };
                            let metric = format!("process.io.{key}");
                            let mut raw = match value {
                                Ok(v) => Point::new(&metric, &entity, v, unit, Kind::Counter),
                                Err(_) => Point::missing(
                                    &metric,
                                    &entity,
                                    unit,
                                    Kind::Counter,
                                    Status::ParseError,
                                ),
                            };
                            let rate = if let Some(n) = raw.value.as_u64() {
                                let old = self.io_baseline.insert(key, (n, b.uptime_s));
                                match old {
                                    Some((v, t)) if n >= v && b.uptime_s > t => Point::new(
                                        format!("{metric}_per_second"),
                                        &entity,
                                        (n - v) as f64 / (b.uptime_s - t),
                                        &format!("{unit}/second"),
                                        Kind::Rate,
                                    ),
                                    Some((v, t)) if n < v || b.uptime_s < t => {
                                        raw.value = Value::Null;
                                        raw.status = Status::Discontinuity;
                                        Point::missing(
                                            format!("{metric}_per_second"),
                                            &entity,
                                            &format!("{unit}/second"),
                                            Kind::Rate,
                                            Status::Discontinuity,
                                        )
                                    }
                                    _ => Point::missing(
                                        format!("{metric}_per_second"),
                                        &entity,
                                        &format!("{unit}/second"),
                                        Kind::Rate,
                                        Status::Stale,
                                    ),
                                }
                            } else {
                                Point::missing(
                                    format!("{metric}_per_second"),
                                    &entity,
                                    &format!("{unit}/second"),
                                    Kind::Rate,
                                    Status::ParseError,
                                )
                            };
                            Collector::stamp(&mut b, raw);
                            Collector::stamp(&mut b, rate);
                        }
                    }
                    let structured = match file {
                        "status" => parsers::trace_status(&text, &entity),
                        "smaps_rollup" => parsers::trace_smaps_rollup(&text, &entity),
                        "sched" => parsers::trace_sched(&text, &entity),
                        _ => Vec::new(),
                    };
                    for point in structured {
                        Collector::stamp(&mut b, point);
                    }
                    Point::new(
                        format!("trace.{file}"),
                        &entity,
                        text.replace('\0', "\\0"),
                        "text",
                        Kind::Gauge,
                    )
                }
                Err(e) => Point::missing(
                    format!("trace.{file}"),
                    &entity,
                    "text",
                    Kind::Gauge,
                    io_status(&e),
                ),
            };
            Collector::stamp(&mut b, point);
        }
        if self.options.extended && start.elapsed() < budget {
            let count = fs::read_dir(dir.join("fd")).map(|entries| entries.take(4097).count());
            let point = match count {
                Ok(n) if n <= 4096 => Point::new(
                    "process.fd_count",
                    &entity,
                    n as u64,
                    "descriptors",
                    Kind::Gauge,
                ),
                Ok(_) => {
                    b.diagnostics
                        .push("fd: skipped_expensive（数量超过 4096）".into());
                    Point::missing(
                        "process.fd_count",
                        &entity,
                        "descriptors",
                        Kind::Gauge,
                        Status::Stale,
                    )
                }
                Err(e) => Point::missing(
                    "process.fd_count",
                    &entity,
                    "descriptors",
                    Kind::Gauge,
                    io_status(&e),
                ),
            };
            Collector::stamp(&mut b, point);
        }
        if self.options.threads {
            if let Ok(entries) = fs::read_dir(dir.join("task")) {
                for (count, entry) in entries.enumerate() {
                    if count >= 4096 || start.elapsed() >= budget || bytes >= FILE_LIMIT {
                        b.diagnostics.push("threads: skipped_expensive".into());
                        break;
                    }
                    let Ok(entry) = entry else { continue };
                    let name = entry.file_name();
                    let Some(tid) = name.to_str().and_then(|n| n.parse::<u32>().ok()) else {
                        continue;
                    };
                    match read_bounded(&entry.path().join("stat"), 8192) {
                        Ok(text) => {
                            bytes += text.len();
                            if let Ok(stat) = parsers::process_stat(&text) {
                                Collector::stamp(
                                    &mut b,
                                    Point::new(
                                        format!("trace.thread.{tid}.state"),
                                        &entity,
                                        stat.state,
                                        "text",
                                        Kind::Gauge,
                                    ),
                                );
                                Collector::stamp(
                                    &mut b,
                                    Point::new(
                                        format!("trace.thread.{tid}.cpu_seconds"),
                                        &entity,
                                        (stat.user_ticks + stat.system_ticks) as f64
                                            / self.collector.ticks_per_second() as f64,
                                        "seconds",
                                        Kind::Gauge,
                                    ),
                                );
                            }
                            Collector::stamp(
                                &mut b,
                                Point::new(
                                    format!("trace.thread.{tid}.stat"),
                                    &entity,
                                    text,
                                    "text",
                                    Kind::Gauge,
                                ),
                            );
                        }
                        Err(e) => b.diagnostics.push(format!("TID {tid}: {e}")),
                    }
                }
            } else {
                b.diagnostics.push("threads: 不可读取".into());
            }
        }
        let (after, _) = self.collector.process(self.options.pid)?;
        if after.starttime_ticks != identity.starttime_ticks {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "采样期间 PID 已复用，丢弃本轮",
            ));
        }
        if start.elapsed() > budget {
            b.complete = false;
            b.samples.clear();
            b.processes.clear();
            b.diagnostics.push("trace 超时，跳过本轮".into());
        } else {
            Collector::stamp(
                &mut b,
                Point::new(
                    "trace.duration_seconds",
                    entity,
                    start.elapsed().as_secs_f64(),
                    "seconds",
                    Kind::Gauge,
                ),
            );
        }
        Ok(b)
    }
}
