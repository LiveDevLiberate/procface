//! 只解析传入文本；读取、权限、采样时钟和预算由采集器负责。
use crate::model::{Kind, Point, Status};
use std::collections::BTreeMap;

pub fn uptime(text: &str) -> Result<f64, String> {
    let n: f64 = text
        .split_whitespace()
        .next()
        .ok_or("uptime 缺失")?
        .parse()
        .map_err(|_| "uptime 非法")?;
    if n.is_finite() && n >= 0.0 {
        Ok(n)
    } else {
        Err("uptime 非法".into())
    }
}
pub fn keyed(text: &str) -> BTreeMap<String, Result<u64, String>> {
    text.lines()
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let key = cols.next()?.trim_end_matches(':').to_string();
            let value = cols
                .next()
                .ok_or_else(|| "数值缺失".to_string())
                .and_then(|s| s.parse::<u64>().map_err(|_| "数值非法".into()));
            Some((key, value))
        })
        .collect()
}
fn field(
    map: &BTreeMap<String, Result<u64, String>>,
    source: &str,
    metric: &str,
    unit: &str,
    kind: Kind,
    scale: u64,
) -> Point {
    match map.get(source) {
        Some(Ok(n)) => match n.checked_mul(scale) {
            Some(v) => Point::new(metric, "system", v, unit, kind),
            None => Point::missing(metric, "system", unit, kind, Status::ParseError),
        },
        Some(Err(_)) => Point::missing(metric, "system", unit, kind, Status::ParseError),
        None => Point::missing(metric, "system", unit, kind, Status::Unsupported),
    }
}
pub fn memory(text: &str) -> Vec<Point> {
    let m = keyed(text);
    let names = [
        ("MemTotal", "total"),
        ("MemAvailable", "available"),
        ("MemFree", "free"),
        ("Buffers", "buffers"),
        ("Cached", "cached"),
        ("SwapTotal", "swap_total"),
        ("SwapFree", "swap_free"),
        ("Slab", "slab"),
        ("SReclaimable", "reclaimable"),
        ("Dirty", "dirty"),
        ("Writeback", "writeback"),
    ];
    let mut out: Vec<_> = names
        .iter()
        .map(|(k, n)| {
            field(
                &m,
                k,
                &format!("memory.{n}_bytes"),
                "bytes",
                Kind::Gauge,
                1024,
            )
        })
        .collect();
    for (total, free, name) in [
        ("MemTotal", "MemAvailable", "usage"),
        ("SwapTotal", "SwapFree", "swap_usage"),
    ] {
        let p = match (m.get(total), m.get(free)) {
            (Some(Ok(t)), Some(Ok(f))) if f <= t => Point::new(
                format!("memory.{name}"),
                "system",
                if *t == 0 {
                    0.0
                } else {
                    100.0 * (*t - *f) as f64 / *t as f64
                },
                "percent",
                Kind::Gauge,
            ),
            (None, _) | (_, None) => Point::missing(
                format!("memory.{name}"),
                "system",
                "percent",
                Kind::Gauge,
                Status::Unsupported,
            ),
            _ => Point::missing(
                format!("memory.{name}"),
                "system",
                "percent",
                Kind::Gauge,
                Status::ParseError,
            ),
        };
        out.push(p);
    }
    out
}
pub fn vm(text: &str) -> Vec<Point> {
    let m = keyed(text);
    [
        ("pgpgin", "page_in_bytes", "bytes", 1024),
        ("pgpgout", "page_out_bytes", "bytes", 1024),
        ("pswpin", "swap_in_pages", "pages", 1),
        ("pswpout", "swap_out_pages", "pages", 1),
        ("pgfault", "faults", "events", 1),
        ("pgmajfault", "major_faults", "events", 1),
    ]
    .iter()
    .map(|(src, name, unit, scale)| {
        field(&m, src, &format!("vm.{name}"), unit, Kind::Counter, *scale)
            .rate(format!("vm.{name}_per_second"))
    })
    .collect()
}
pub fn load(text: &str) -> Vec<Point> {
    let cols: Vec<_> = text.split_whitespace().collect();
    let mut out = Vec::new();
    for (i, name) in ["load.1m", "load.5m", "load.15m"].iter().enumerate() {
        match cols
            .get(i)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.0)
        {
            Some(v) => out.push(Point::new(*name, "system", v, "tasks", Kind::Gauge)),
            None => out.push(Point::missing(
                *name,
                "system",
                "tasks",
                Kind::Gauge,
                Status::ParseError,
            )),
        }
    }
    let tasks: Vec<_> = cols.get(3).unwrap_or(&"").split('/').collect();
    for (i, name) in ["load.running", "load.processes"].iter().enumerate() {
        out.push(match tasks.get(i).and_then(|v| v.parse::<u64>().ok()) {
            Some(v) => Point::new(*name, "system", v, "tasks", Kind::Gauge),
            None => Point::missing(*name, "system", "tasks", Kind::Gauge, Status::ParseError),
        });
    }
    out
}
#[derive(Clone, Debug)]
pub struct Cpu {
    pub entity: String,
    pub ticks: [u64; 8],
}
pub fn stat(text: &str) -> (Vec<Point>, Vec<Cpu>) {
    let mut points = Vec::new();
    let mut cpus = Vec::new();
    for line in text.lines() {
        let cols: Vec<_> = line.split_whitespace().collect();
        if cols.is_empty() {
            continue;
        }
        if cols[0] == "cpu"
            || cols[0]
                .strip_prefix("cpu")
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            let mut ticks = [0; 8];
            let mut valid = cols.len() >= 5;
            for (i, item) in ticks.iter_mut().enumerate() {
                if let Some(v) = cols.get(i + 1) {
                    match v.parse::<u64>() {
                        Ok(v) => *item = v,
                        Err(_) => valid = false,
                    }
                }
            }
            if valid {
                cpus.push(Cpu {
                    entity: cols[0].to_string(),
                    ticks,
                });
            } else {
                points.push(Point::missing(
                    "cpu.usage",
                    cols[0],
                    "percent",
                    Kind::Gauge,
                    Status::ParseError,
                ));
            }
        } else {
            let mapping = match cols[0] {
                "ctxt" => Some(("cpu.context_switches", "events", Kind::Counter)),
                "processes" => Some(("cpu.forks", "events", Kind::Counter)),
                "procs_running" => Some(("cpu.running", "tasks", Kind::Gauge)),
                "procs_blocked" => Some(("cpu.blocked", "tasks", Kind::Gauge)),
                _ => None,
            };
            if let Some((name, unit, kind)) = mapping {
                let mut p = match cols.get(1).and_then(|s| s.parse::<u64>().ok()) {
                    Some(v) => Point::new(name, "system", v, unit, kind),
                    None => Point::missing(name, "system", unit, kind, Status::ParseError),
                };
                if kind == Kind::Counter {
                    p = p.rate(format!("{name}_per_second"));
                }
                points.push(p);
            }
        }
    }
    if cpus.is_empty() && !points.iter().any(|p| p.metric == "cpu.usage") {
        points.push(Point::missing(
            "cpu.usage",
            "cpu",
            "percent",
            Kind::Gauge,
            Status::ParseError,
        ));
    }
    for (metric, unit, kind) in [
        ("cpu.context_switches", "events", Kind::Counter),
        ("cpu.forks", "events", Kind::Counter),
        ("cpu.running", "tasks", Kind::Gauge),
        ("cpu.blocked", "tasks", Kind::Gauge),
    ] {
        if !points.iter().any(|p| p.metric == metric) {
            let mut point = Point::missing(metric, "system", unit, kind, Status::Unsupported);
            if kind == Kind::Counter {
                point = point.rate(format!("{metric}_per_second"));
            }
            points.push(point);
        }
    }
    (points, cpus)
}
pub fn disks(text: &str) -> Vec<Point> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let c: Vec<_> = line.split_whitespace().collect();
        let entity = c.get(2).copied().unwrap_or("unknown");
        let specs = [
            (3, "read_operations", "operations", 1, Kind::Counter),
            (5, "read_bytes", "bytes", 512, Kind::Counter),
            (6, "read_time_ms", "milliseconds", 1, Kind::Counter),
            (7, "write_operations", "operations", 1, Kind::Counter),
            (9, "write_bytes", "bytes", 512, Kind::Counter),
            (10, "write_time_ms", "milliseconds", 1, Kind::Counter),
            (11, "in_flight", "operations", 1, Kind::Gauge),
            (12, "io_time_ms", "milliseconds", 1, Kind::Counter),
            (13, "weighted_io_time_ms", "milliseconds", 1, Kind::Counter),
        ];
        for (i, name, unit, scale, kind) in specs {
            let mut p = match c
                .get(i)
                .and_then(|s| s.parse::<u64>().ok())
                .and_then(|v| v.checked_mul(scale))
            {
                Some(v) => Point::new(format!("disk.{name}"), entity, v, unit, kind),
                None => Point::missing(
                    format!("disk.{name}"),
                    entity,
                    unit,
                    kind,
                    Status::ParseError,
                ),
            };
            if kind == Kind::Counter {
                p = p.rate(format!("disk.{name}_per_second"));
            }
            out.push(p);
        }
    }
    out
}
pub fn network(text: &str) -> Vec<Point> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((entity, data)) = line.rsplit_once(':') else {
            continue;
        };
        let c: Vec<_> = data.split_whitespace().collect();
        for (i, name, unit) in [
            (0, "rx_bytes", "bytes"),
            (1, "rx_packets", "packets"),
            (2, "rx_errors", "events"),
            (3, "rx_dropped", "packets"),
            (8, "tx_bytes", "bytes"),
            (9, "tx_packets", "packets"),
            (10, "tx_errors", "events"),
            (11, "tx_dropped", "packets"),
        ] {
            let p = match c.get(i).and_then(|s| s.parse::<u64>().ok()) {
                Some(v) => Point::new(
                    format!("network.{name}"),
                    entity.trim(),
                    v,
                    unit,
                    Kind::Counter,
                ),
                None => Point::missing(
                    format!("network.{name}"),
                    entity.trim(),
                    unit,
                    Kind::Counter,
                    Status::ParseError,
                ),
            };
            out.push(p.rate(format!("network.{name}_per_second")));
        }
    }
    out
}
#[derive(Clone, Debug)]
pub struct ProcessStat {
    pub pid: u32,
    pub name: String,
    pub state: String,
    pub user_ticks: u64,
    pub system_ticks: u64,
    pub threads: u64,
    pub starttime_ticks: u64,
    pub virtual_bytes: u64,
    pub rss_pages: u64,
}
pub fn process_stat(text: &str) -> Result<ProcessStat, String> {
    let open = text.find('(').ok_or("进程名称缺失")?;
    let close = text.rfind(')').ok_or("进程名称未闭合")?;
    if close <= open {
        return Err("进程名称非法".into());
    }
    let pid = text[..open].trim().parse().map_err(|_| "PID 非法")?;
    let c: Vec<_> = text[close + 1..].split_whitespace().collect();
    let n = |i: usize| -> Result<u64, String> {
        c.get(i)
            .ok_or_else(|| "stat 字段缺失".to_string())?
            .parse()
            .map_err(|_| "stat 字段非法".into())
    };
    Ok(ProcessStat {
        pid,
        name: text[open + 1..close].into(),
        state: c.first().ok_or("state 缺失")?.to_string(),
        user_ticks: n(11)?,
        system_ticks: n(12)?,
        threads: n(17)?,
        starttime_ticks: n(19)?,
        virtual_bytes: n(20)?,
        rss_pages: n(21)?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_edges() {
        assert!(uptime("NaN 0").is_err());
        assert!(uptime("-1 0").is_err());
        let m = memory("MemTotal: 100 kB\nMemFree: bad kB\n");
        assert_eq!(m[0].value.as_u64(), Some(102400));
        assert_eq!(m[1].status, Status::Unsupported);
        assert_eq!(m[2].status, Status::ParseError);
        assert_eq!(
            m.iter()
                .find(|p| p.metric == "memory.usage")
                .unwrap()
                .status,
            Status::Unsupported
        );
        let d = disks("8 0 sda 1 0 4 6 2 0 8 9 0 10 11");
        assert_eq!(d[1].value.as_u64(), Some(2048));
        let n = network("lo: 10 2 0 0 0 0 0 0 20 4 0 0 0 0 0 0");
        assert_eq!(n[4].value.as_u64(), Some(20));
        assert_eq!(
            stat("cpu 1 2 3 4 5 6 7 8 99 88").1[0]
                .ticks
                .iter()
                .sum::<u64>(),
            36
        );
        let s = process_stat("42 (odd ) name) R 1 0 0 0 0 0 0 0 0 0 10 20 0 0 0 0 2 0 123 4096 7")
            .unwrap();
        assert_eq!(s.name, "odd ) name");
        assert_eq!(s.starttime_ticks, 123);
        assert_eq!(s.user_ticks, 10);
        assert_eq!(s.rss_pages, 7);
    }
}
