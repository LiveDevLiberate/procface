use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};

pub const SCHEMA_VERSION: u32 = 1;
pub const API_VERSION: u32 = 1;
pub const TSV_HEADER: &str = "schema_version\tsession_id\tsequence\tuptime_s\ttimestamp_unix\tmetric\tentity\tvalue\tunit\tkind\tstatus";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Unsupported,
    PermissionDenied,
    ParseError,
    Exited,
    Stale,
    Discontinuity,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Gauge,
    Counter,
    Rate,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub schema_version: u32,
    pub session_id: String,
    pub sequence: u64,
    pub uptime_s: f64,
    pub timestamp_unix: Option<f64>,
    pub metric: String,
    pub entity: String,
    pub value: Value,
    pub unit: String,
    pub kind: Kind,
    pub status: Status,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub starttime_ticks: u64,
}
impl ProcessIdentity {
    pub fn entity(&self) -> String {
        format!("process:{}:{}", self.pid, self.starttime_ticks)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub identity: ProcessIdentity,
    pub name: String,
    pub state: String,
    pub uid: u32,
    pub rss_bytes: u64,
    pub threads: u64,
    pub cpu_seconds: f64,
    pub cpu_percent: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SampleBatch {
    /// 仅 daemon 缓存使用；CLI 与 SQLite 序列化保持可读并独立于内存字典。
    #[serde(skip)]
    pub wire: Option<std::sync::Arc<crate::compact::EncodedBatch>>,
    pub schema_version: u32,
    pub session_id: String,
    pub sequence: u64,
    pub uptime_s: f64,
    pub timestamp_unix: Option<f64>,
    pub group: String,
    pub samples: Vec<Sample>,
    pub processes: Vec<ProcessInfo>,
    pub complete: bool,
    pub diagnostics: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Point {
    pub metric: String,
    pub entity: String,
    pub value: Value,
    pub unit: String,
    pub kind: Kind,
    pub status: Status,
    pub rate_metric: Option<String>,
}
impl Point {
    pub fn new(
        metric: impl Into<String>,
        entity: impl Into<String>,
        value: impl Into<Value>,
        unit: &str,
        kind: Kind,
    ) -> Self {
        Self {
            metric: metric.into(),
            entity: entity.into(),
            value: value.into(),
            unit: unit.into(),
            kind,
            status: Status::Ok,
            rate_metric: None,
        }
    }
    pub fn missing(
        metric: impl Into<String>,
        entity: impl Into<String>,
        unit: &str,
        kind: Kind,
        status: Status,
    ) -> Self {
        let mut p = Self::new(metric, entity, Value::Null, unit, kind);
        p.status = status;
        p
    }
    pub fn rate(mut self, name: impl Into<String>) -> Self {
        self.rate_metric = Some(name.into());
        self
    }
}
pub fn random_id() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn cell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}
pub fn write_tsv(out: &mut impl Write, s: &Sample) -> io::Result<()> {
    let value = match &s.value {
        Value::Null => String::new(),
        Value::String(v) => cell(v),
        v => v.to_string(),
    };
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        s.schema_version,
        cell(&s.session_id),
        s.sequence,
        s.uptime_s,
        s.timestamp_unix.map(|v| v.to_string()).unwrap_or_default(),
        cell(&s.metric),
        cell(&s.entity),
        value,
        cell(&s.unit),
        serde_json::to_value(s.kind).unwrap().as_str().unwrap(),
        serde_json::to_value(s.status).unwrap().as_str().unwrap()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_precision_and_tsv_contract() {
        let s = Sample {
            schema_version: 1,
            session_id: "s".into(),
            sequence: 1,
            uptime_s: 4.25,
            timestamp_unix: None,
            metric: "network.rx_bytes".into(),
            entity: "eth0".into(),
            value: u64::MAX.into(),
            unit: "bytes".into(),
            kind: Kind::Counter,
            status: Status::Ok,
        };
        let encoded = serde_json::to_string(&s).unwrap();
        let decoded: Sample = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.value.as_u64(), Some(u64::MAX));
        let mut out = Vec::new();
        write_tsv(&mut out, &s).unwrap();
        let line = String::from_utf8(out).unwrap();
        assert_eq!(line.trim_end().split('\t').count(), 11);
        assert!(line.contains("18446744073709551615"));
        assert_eq!(TSV_HEADER.split('\t').nth(5), Some("metric"));
    }
}
