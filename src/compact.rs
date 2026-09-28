//! API 紧凑批次。指标编号由追加式发布目录提供，不按采集顺序分配。
//! 实体目录由 session 持有；每个编码批次保留自身的字典引用。
use crate::model::{Kind, SampleBatch, Status};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    io,
    sync::{Arc, Weak},
};

pub const WIRE_SCHEMA: &str = "procface-compact-v1";
pub const GROUPS: [&str; 3] = ["system", "process", "trace"];
pub const STATUSES: [&str; 7] = [
    "ok",
    "unsupported",
    "permission_denied",
    "parse_error",
    "exited",
    "stale",
    "discontinuity",
];
pub const PROCESS_FIELDS: [&str; 12] = [
    "entity_id",
    "pid",
    "starttime_ticks",
    "name",
    "state",
    "uid",
    "rss_bytes",
    "threads",
    "cpu_seconds",
    "cpu_percent",
    "sample_mask",
    "cpu_status_id",
];
// 位顺序是协议的一部分。virtual_bytes 不在 ProcessInfo 中，仍按普通样本发送。
pub const PROCESS_METRICS: [&str; 7] = [
    "process.pid",
    "process.name",
    "process.state",
    "process.rss_bytes",
    "process.threads",
    "process.cpu_seconds",
    "process.cpu_usage",
];

#[derive(Clone, Debug, Serialize)]
pub struct Metric(pub u64, pub String, pub String, pub Kind);

/// 发布目录必须显式给出编号；未知指标返回错误，不临时分配跨版本不稳定编号。
pub struct Catalog(HashMap<String, Metric>);
impl Catalog {
    pub fn builtin() -> io::Result<Self> {
        let mut metrics = Vec::new();
        for line in include_str!("metric-catalog.tsv").lines().skip(1) {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 4 {
                return Err(io::Error::other("指标目录列数错误"));
            }
            let kind = match fields[3] {
                "gauge" => Kind::Gauge,
                "counter" => Kind::Counter,
                "rate" => Kind::Rate,
                _ => return Err(io::Error::other("指标目录类型错误")),
            };
            metrics.push(Metric(
                fields[0].parse().map_err(io::Error::other)?,
                fields[1].into(),
                fields[2].into(),
                kind,
            ));
        }
        Self::new(metrics)
    }
    pub fn new(metrics: Vec<Metric>) -> io::Result<Self> {
        let mut names = HashMap::new();
        let mut ids = std::collections::HashSet::new();
        for metric in metrics {
            if metric.0 == 0
                || metric.0 >= 1u64 << 32
                || !ids.insert(metric.0)
                || names.contains_key(&metric.1)
            {
                return Err(io::Error::other("重复或非法指标编号"));
            }
            names.insert(metric.1.clone(), metric);
        }
        Ok(Self(names))
    }
    pub fn dictionary(&self) -> Vec<&Metric> {
        let mut metrics: Vec<_> = self.0.values().collect();
        metrics.sort_by_key(|m| m.0);
        metrics
    }
    fn resolve(&self, name: &str) -> Option<Metric> {
        self.0.get(name).cloned().or_else(|| {
            // 保留整个 u32 TID 编号区间，线程编号不占用静态指标目录。
            let tid = name.strip_prefix("trace.thread.")?.strip_suffix(".stat")?;
            let number = tid.parse::<u32>().ok()?;
            if number == 0 || number.to_string() != tid {
                return None;
            }
            Some(Metric(
                (1u64 << 32) + u64::from(number),
                name.into(),
                "text".into(),
                Kind::Gauge,
            ))
        })
    }
}

#[derive(Debug, Serialize)]
struct Entity(u64, String);

#[derive(Default)]
pub struct Entities {
    entries: HashMap<String, Weak<Entity>>,
    last_id: u64,
}
impl Entities {
    fn get(&mut self, name: &str) -> io::Result<Arc<Entity>> {
        if let Some(entry) = self.entries.get(name).and_then(Weak::upgrade) {
            return Ok(entry);
        }
        self.last_id = self
            .last_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("实体编号耗尽"))?;
        let entry = Arc::new(Entity(self.last_id, name.to_owned()));
        self.entries.insert(name.to_owned(), Arc::downgrade(&entry));
        Ok(entry)
    }
    /// 当前快照、历史、队列、导出持有 EncodedBatch 时，对应条目不会被回收。
    pub fn collect(&mut self) {
        self.entries.retain(|_, entry| entry.strong_count() != 0);
    }
}

#[derive(Debug)]
pub struct EncodedBatch {
    pub sequence: u64,
    uptime: f64,
    timestamp_unix: Option<f64>,
    group: u8,
    complete: bool,
    diagnostics: Vec<String>,
    metrics: BTreeMap<u64, Metric>,
    entities: BTreeMap<u64, Arc<Entity>>,
    samples: Vec<Value>,
    processes: Vec<Value>,
}
impl EncodedBatch {
    /// 共享目录按每批次保守重复计量，避免低估缓存预算。
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.diagnostics.capacity() * std::mem::size_of::<String>()
            + self.diagnostics.iter().map(String::capacity).sum::<usize>()
            + self
                .metrics
                .values()
                .map(|m| std::mem::size_of::<Metric>() + m.1.capacity() + m.2.capacity() + 64)
                .sum::<usize>()
            + self
                .entities
                .values()
                .map(|e| std::mem::size_of::<Entity>() + e.1.capacity() + 64)
                .sum::<usize>()
            + (self.samples.capacity() + self.processes.capacity()) * std::mem::size_of::<Value>()
            + self
                .samples
                .iter()
                .chain(&self.processes)
                .map(value_heap_bytes)
                .sum::<usize>()
    }
    /// 每个批次携带其引用的字典子集，支持窗口中任意位置开始的独立解码。
    /// 字典位于 samples/processes 之前；接收方可覆盖相同定义。
    pub fn value(&self) -> Value {
        let entities: Vec<_> = self.entities.values().map(Arc::as_ref).collect();
        json!([self.sequence, self.uptime, self.timestamp_unix, self.group, self.complete,
            self.diagnostics, {"metrics":self.metrics.values().collect::<Vec<_>>(), "entities":entities},
            self.samples, self.processes])
    }
    pub fn envelope(&self, session: &str) -> Value {
        json!({"wire_schema":WIRE_SCHEMA,"schema_version":1,"session_id":session,
            "dictionary":{"groups":GROUPS,"statuses":STATUSES,"process_fields":PROCESS_FIELDS},
            "batches":[self.value()]})
    }
}

/// JSON 的嵌套文本、数组和对象也占缓存，不能只计最外层 Value。
pub fn value_heap_bytes(value: &Value) -> usize {
    match value {
        Value::String(s) => s.capacity(),
        Value::Array(values) => {
            values.capacity() * std::mem::size_of::<Value>()
                + values.iter().map(value_heap_bytes).sum::<usize>()
        }
        Value::Object(values) => values
            .iter()
            .map(|(k, v)| {
                64 + std::mem::size_of::<(String, Value)>() + k.capacity() + value_heap_bytes(v)
            })
            .sum(),
        _ => 0,
    }
}

pub fn encode(
    b: &SampleBatch,
    catalog: &Catalog,
    directory: &mut Entities,
) -> io::Result<EncodedBatch> {
    let group = GROUPS
        .iter()
        .position(|g| *g == b.group)
        .ok_or_else(|| io::Error::other("未知批次分组"))? as u8;
    let mut encoded = EncodedBatch {
        sequence: b.sequence,
        uptime: b.uptime_s,
        timestamp_unix: b.timestamp_unix,
        group,
        complete: b.complete,
        diagnostics: b.diagnostics.clone(),
        metrics: BTreeMap::new(),
        entities: BTreeMap::new(),
        samples: Vec::new(),
        processes: Vec::new(),
    };
    let mut process_rows = HashMap::new();
    for p in &b.processes {
        let name = p.identity.entity();
        if process_rows.contains_key(&name) {
            return Err(io::Error::other("重复进程身份"));
        }
        let entity = directory.get(&name)?;
        process_rows.insert(name, encoded.processes.len());
        encoded.processes.push(json!([
            entity.0,
            p.identity.pid,
            p.identity.starttime_ticks,
            p.name,
            p.state,
            p.uid,
            p.rss_bytes,
            p.threads,
            p.cpu_seconds,
            p.cpu_percent,
            0,
            0
        ]));
        encoded.entities.insert(entity.0, entity);
    }
    for s in &b.samples {
        let metric = catalog
            .resolve(&s.metric)
            .ok_or_else(|| io::Error::other(format!("未登记指标: {}", s.metric)))?;
        if metric.2 != s.unit || metric.3 != s.kind {
            return Err(io::Error::other("指标目录单位或类型不一致"));
        }
        // 即使进程样本被合并，字典仍能还原其名称、单位与类型。
        encoded.metrics.insert(metric.0, metric.clone());
        let status = status_id(s.status);
        let mut merged = false;
        if let (Some(row), Some(bit)) = (
            process_rows.get(&s.entity),
            PROCESS_METRICS.iter().position(|m| *m == s.metric),
        ) {
            let process = &mut encoded.processes[*row];
            let column = [1, 3, 4, 6, 7, 8, 9][bit];
            let mask = process[10].as_u64().unwrap();
            if mask & (1 << bit) == 0 && process[column] == s.value && (status == 0 || bit == 6) {
                process[10] = (mask | (1 << bit)).into();
                if bit == 6 {
                    process[11] = status.into();
                }
                merged = true;
            }
        }
        if !merged {
            let entity = directory.get(&s.entity)?;
            encoded
                .samples
                .push(json!([metric.0, entity.0, s.value, status]));
            encoded.entities.insert(entity.0, entity);
        }
    }
    Ok(encoded)
}

fn status_id(s: Status) -> u8 {
    match s {
        Status::Ok => 0,
        Status::Unsupported => 1,
        Status::PermissionDenied => 2,
        Status::ParseError => 3,
        Status::Exited => 4,
        Status::Stale => 5,
        Status::Discontinuity => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ProcessIdentity, ProcessInfo, Sample};

    fn batch() -> SampleBatch {
        let p = ProcessInfo {
            identity: ProcessIdentity {
                pid: 42,
                starttime_ticks: u64::MAX,
            },
            name: "worker ) 名称".into(),
            state: "R".into(),
            uid: 1000,
            rss_bytes: 123456,
            threads: 2,
            cpu_seconds: 0.125,
            cpu_percent: None,
        };
        let values = [
            json!(42),
            json!(p.name),
            json!(p.state),
            json!(p.rss_bytes),
            json!(2),
            json!(0.125),
            Value::Null,
        ];
        let units = [
            "id", "text", "text", "bytes", "threads", "seconds", "percent",
        ];
        let mut samples: Vec<_> = PROCESS_METRICS
            .iter()
            .enumerate()
            .map(|(i, m)| Sample {
                schema_version: 1,
                session_id: "session".into(),
                sequence: 7,
                uptime_s: 8.5,
                timestamp_unix: None,
                metric: (*m).into(),
                entity: p.identity.entity(),
                value: values[i].clone(),
                unit: units[i].into(),
                kind: if i == 5 { Kind::Counter } else { Kind::Gauge },
                status: if i == 6 { Status::Stale } else { Status::Ok },
            })
            .collect();
        for (name, value) in [
            (
                "trace.io",
                json!({"count":u64::MAX,"list":[true,"文本",null]}),
            ),
            ("process.virtual_bytes", json!(u64::MAX)),
        ] {
            let mut s = samples[0].clone();
            s.metric = name.into();
            s.value = value;
            samples.push(s);
        }
        SampleBatch {
            wire: None,
            schema_version: 1,
            session_id: "session".into(),
            sequence: 7,
            uptime_s: 8.5,
            timestamp_unix: None,
            group: "trace".into(),
            samples,
            processes: vec![p],
            complete: false,
            diagnostics: vec!["部分文件不可读取".into()],
        }
    }
    fn catalog(b: &SampleBatch) -> Catalog {
        Catalog::new(
            b.samples
                .iter()
                .enumerate()
                .map(|(i, s)| Metric(i as u64 + 1, s.metric.clone(), s.unit.clone(), s.kind))
                .collect(),
        )
        .unwrap()
    }
    // 独立按线格式还原样本（不调用编码器内部函数），检验合并和筛选是否丢信息。
    fn expanded(w: &Value) -> Vec<Value> {
        let metrics: HashMap<_, _> = w[6]["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| (m[0].as_u64().unwrap(), m))
            .collect();
        let entities: HashMap<_, _> = w[6]["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e[0].as_u64().unwrap(), e[1].clone()))
            .collect();
        let mut rows = Vec::new();
        let mut append = |metric: &Value, entity: &Value, value: &Value, status: usize| {
            rows.push(json!([
                metric[1],
                entity,
                value,
                metric[2],
                metric[3],
                STATUSES[status]
            ]));
        };
        for s in w[7].as_array().unwrap() {
            append(
                metrics[&s[0].as_u64().unwrap()],
                &entities[&s[1].as_u64().unwrap()],
                &s[2],
                s[3].as_u64().unwrap() as usize,
            );
        }
        for p in w[8].as_array().unwrap() {
            for (bit, name) in PROCESS_METRICS.iter().enumerate() {
                if p[10].as_u64().unwrap() & (1 << bit) == 0 {
                    continue;
                }
                let metric = metrics.values().find(|m| m[1] == *name).unwrap();
                append(
                    metric,
                    &entities[&p[0].as_u64().unwrap()],
                    &p[[1, 3, 4, 6, 7, 8, 9][bit]],
                    if bit == 6 {
                        p[11].as_u64().unwrap() as usize
                    } else {
                        0
                    },
                );
            }
        }
        rows.sort_by_key(Value::to_string);
        rows
    }
    fn expected(b: &SampleBatch) -> Vec<Value> {
        let mut v: Vec<_> = b
            .samples
            .iter()
            .map(|s| json!([s.metric, s.entity, s.value, s.unit, s.kind, s.status]))
            .collect();
        v.sort_by_key(Value::to_string);
        v
    }
    #[test]
    fn process_dedup_roundtrip_filter_and_trace_precision() {
        let mut b = batch();
        let c = catalog(&b);
        let mut d = Entities::default();
        let full = encode(&b, &c, &mut d).unwrap();
        let w: Value = serde_json::from_str(&full.value().to_string()).unwrap();
        assert_eq!(w.as_array().unwrap().len(), 9);
        assert_eq!(w[7].as_array().unwrap().len(), 2);
        assert_eq!(w[8][0][2].as_u64(), Some(u64::MAX));
        assert_eq!(w[4], false);
        assert_eq!(w[5], json!(b.diagnostics));
        assert_eq!(expanded(&w), expected(&b));
        assert!(
            full.envelope("session").to_string().len() < serde_json::to_string(&b).unwrap().len()
        );
        b.samples.retain(|s| s.metric == "process.cpu_usage");
        assert_eq!(
            expanded(&encode(&b, &c, &mut d).unwrap().value()),
            expected(&b)
        );
        b.samples[0].status = Status::Exited;
        b.samples[0].value = json!(123);
        assert_eq!(
            expanded(&encode(&b, &c, &mut d).unwrap().value()),
            expected(&b)
        );
    }
    #[test]
    fn dictionary_lifetime_and_ids_do_not_depend_on_sample_order() {
        let mut b = batch();
        let c = catalog(&b);
        let mut d = Entities::default();
        let a = encode(&b, &c, &mut d).unwrap();
        b.samples.reverse();
        let second = encode(&b, &c, &mut d).unwrap();
        assert_eq!(a.value()[6], second.value()[6]);
        let old_id = a.value()[8][0][0].as_u64().unwrap();
        drop(a);
        d.collect();
        assert_eq!(d.entries.len(), 1);
        assert_eq!(expanded(&second.value()), expected(&b));
        drop(second);
        d.collect();
        assert!(d.entries.is_empty());
        let new = encode(&b, &c, &mut d).unwrap();
        assert!(new.value()[8][0][0].as_u64().unwrap() > old_id);
        let mut other = Entities::default();
        assert_eq!(
            new.value()[6]["metrics"],
            encode(&b, &c, &mut other).unwrap().value()[6]["metrics"]
        );
    }
    #[test]
    fn invalid_catalog_or_metadata_is_not_silently_encoded() {
        let mut b = batch();
        let c = catalog(&b);
        let mut d = Entities::default();
        b.samples[0].unit = "wrong".into();
        assert!(encode(&b, &c, &mut d).is_err());
        b.samples[0].metric = "unknown".into();
        assert!(encode(&b, &c, &mut d).is_err());
        assert!(Catalog::new(vec![
            Metric(1, "a".into(), "".into(), Kind::Gauge),
            Metric(1, "b".into(), "".into(), Kind::Gauge)
        ])
        .is_err());
    }

    #[test]
    fn builtin_catalog_covers_parsers_and_dynamic_threads() {
        use crate::parsers;
        let catalog = Catalog::builtin().unwrap();
        let mut points = parsers::memory("");
        points.extend(parsers::vm(""));
        points.extend(parsers::load(""));
        points.extend(
            parsers::stat("cpu 1 2 3 4\nctxt 1\nprocesses 2\nprocs_running 1\nprocs_blocked 0").0,
        );
        points.extend(parsers::disks("8 0 sda 1 2 3 4 5 6 7 8 9 10 11"));
        points.extend(parsers::network(
            "eth0: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16",
        ));
        for point in points {
            let entry = catalog
                .resolve(&point.metric)
                .expect("解析器指标必须已登记");
            assert_eq!((entry.2, entry.3), (point.unit.clone(), point.kind));
            if let Some(rate) = point.rate_metric {
                let entry = catalog.resolve(&rate).unwrap();
                assert_eq!(
                    (entry.2, entry.3),
                    (format!("{}/second", point.unit), Kind::Rate)
                );
            }
        }
        assert_eq!(catalog.resolve("cpu.usage").unwrap().0, 10);
        assert_eq!(
            catalog.resolve("trace.thread.4294967295.stat").unwrap().0,
            8589934591
        );
        for name in [
            "trace.thread.0.stat",
            "trace.thread.01.stat",
            "trace.thread.4294967296.stat",
            "trace.thread.x.stat",
        ] {
            assert!(catalog.resolve(name).is_none());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn fixture_system_process_and_trace_use_published_catalog() {
        use crate::{
            collector::{current_uid, Collector},
            trace::{TraceOptions, Tracer},
        };
        use std::fs;
        let root = std::env::temp_dir().join(format!(
            "procface-wire-{}",
            crate::model::random_id().unwrap()
        ));
        fs::create_dir_all(root.join("100/task/100")).unwrap();
        fs::write(root.join("uptime"), "10 0\n").unwrap();
        let stat = "100 (worker) R 1 0 0 0 0 0 0 0 0 0 10 20 0 0 0 0 2 0 123 4096 7\n";
        fs::write(root.join("100/stat"), stat).unwrap();
        fs::write(root.join("100/task/100/stat"), stat).unwrap();
        fs::write(
            root.join("100/status"),
            format!(
                "Name:\tworker\nUid:\t{}\t{}\t{}\t{}\n",
                current_uid(),
                current_uid(),
                current_uid(),
                current_uid()
            ),
        )
        .unwrap();
        fs::write(root.join("100/io"),"rchar: 1\nwchar: 2\nsyscr: 3\nsyscw: 4\nread_bytes: 5\nwrite_bytes: 6\ncancelled_write_bytes: 7\n").unwrap();
        let catalog = Catalog::builtin().unwrap();
        let mut entities = Entities::default();
        let mut collector = Collector::new(root.clone()).unwrap();
        let groups = ["time", "cpu", "memory", "load", "vm", "disk", "network"].map(String::from);
        let system = collector.system(&groups).unwrap();
        encode(&system, &catalog, &mut entities).unwrap();
        let processes = collector.processes().unwrap();
        assert_eq!(processes.processes.len(), 1);
        assert_eq!(
            expanded(&encode(&processes, &catalog, &mut entities).unwrap().value()),
            expected(&processes)
        );
        let mut tracer = Tracer::new(
            root.clone(),
            TraceOptions {
                pid: 100,
                extended: true,
                threads: true,
                sensitive: cfg!(feature = "diagnostic"),
                budget_ms: 10000,
            },
            current_uid(),
        )
        .unwrap();
        let trace = tracer.sample().unwrap();
        assert!(trace
            .samples
            .iter()
            .any(|s| s.metric == "trace.thread.100.stat"));
        assert_eq!(
            expanded(&encode(&trace, &catalog, &mut entities).unwrap().value()),
            expected(&trace)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
