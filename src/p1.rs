//! P1 系统来源的纯文本解析；计数器基线复用 Collector。
use crate::model::{Kind, Point, Status};

pub fn pressure(text: &str) -> Vec<Point> {
    let mut out = Vec::new();
    for scope in ["some", "full"] {
        let fields = text.lines().find_map(|line| {
            let mut c = line.split_whitespace();
            (c.next() == Some(scope)).then(|| c.collect::<Vec<_>>())
        });
        for field in ["avg10", "avg60", "avg300", "total"] {
            let counter = field == "total";
            let name = format!("pressure.{scope}.{field}");
            let unit = if counter { "microseconds" } else { "percent" };
            let kind = if counter { Kind::Counter } else { Kind::Gauge };
            let value = fields.as_ref().and_then(|c| {
                c.iter().find_map(|v| {
                    let (key, value) = v.split_once('=')?;
                    (key == field).then_some(value)
                })
            });
            let parsed = value.and_then(|s| {
                if counter {
                    s.parse::<u64>().ok().map(serde_json::Value::from)
                } else {
                    s.parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
                        .map(serde_json::Value::from)
                }
            });
            let mut point = match parsed {
                Some(v) => Point::new(&name, "system", v, unit, kind),
                None => Point::missing(
                    &name,
                    "system",
                    unit,
                    kind,
                    if value.is_some() {
                        Status::ParseError
                    } else {
                        Status::Unsupported
                    },
                ),
            };
            if counter {
                point = point.rate(format!("{name}_per_second"));
            }
            out.push(point);
        }
    }
    out
}

pub fn interrupts(text: &str) -> Vec<Point> {
    irq_table(text, "interrupts")
}
pub fn softirqs(text: &str) -> Vec<Point> {
    irq_table(text, "softirq")
}

fn irq_table(text: &str, group: &str) -> Vec<Point> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let cpus: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    let valid_header = !cpus.is_empty()
        && cpus.iter().all(|c| {
            c.strip_prefix("CPU")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        && cpus.iter().collect::<std::collections::HashSet<_>>().len() == cpus.len();
    if !valid_header {
        return vec![Point::missing(
            format!("{group}.available"),
            "system",
            "boolean",
            Kind::Gauge,
            Status::ParseError,
        )];
    }
    let mut out = Vec::new();
    for line in lines {
        let (irq, values) = if let Some((irq, values)) = line.split_once(':') {
            (irq.trim(), values)
        } else if group == "softirq" {
            let mut fields = line.split_whitespace();
            let Some(irq) = fields.next() else { continue };
            (
                irq,
                line.get(line.find(irq).unwrap_or(0) + irq.len()..)
                    .unwrap_or(""),
            )
        } else {
            out.push(Point::missing(
                format!("{group}.available"),
                "system",
                "boolean",
                Kind::Gauge,
                Status::ParseError,
            ));
            continue;
        };
        let cols: Vec<_> = values.split_whitespace().collect();
        // ERR/MIS 等为全局计数，不应伪造为每 CPU 数据。
        let global = group == "interrupts" && matches!(irq.trim(), "ERR" | "MIS");
        let entities: Vec<_> = if global { vec!["all"] } else { cpus.clone() };
        for (i, cpu) in entities.iter().enumerate() {
            let name = format!("{group}.count");
            let entity = format!("{group}:{}:{cpu}", irq.trim());
            let p = match cols.get(i).and_then(|s| s.parse::<u64>().ok()) {
                Some(v) => Point::new(&name, entity, v, "events", Kind::Counter),
                None => Point::missing(&name, entity, "events", Kind::Counter, Status::ParseError),
            };
            out.push(p.rate(format!("{name}_per_second")));
        }
    }
    if out.is_empty() {
        out.push(Point::new(
            format!("{group}.available"),
            "system",
            true,
            "boolean",
            Kind::Gauge,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn psi_fields_and_precision() {
        let p = pressure("some avg10=1.25 avg60=nan avg300=0 total=18446744073709551615\n");
        assert_eq!(p[0].value, 1.25);
        assert_eq!(p[1].status, Status::ParseError);
        assert_eq!(p[3].value.as_u64(), Some(u64::MAX));
        assert!(p[4..].iter().all(|p| p.status == Status::Unsupported));
    }
    #[test]
    fn irq_cpu_columns_and_global_counters() {
        let p = interrupts(" CPU0 CPU3\n 24: 10 20 PCI device\n ERR: 7\n NEW: 5 broken\n");
        assert_eq!(p[1].entity, "interrupts:24:CPU3");
        assert_eq!(p[2].entity, "interrupts:ERR:all");
        assert_eq!(p[2].value, 7);
        assert_eq!(p[4].status, Status::ParseError);
        assert_eq!(softirqs("bad")[0].status, Status::ParseError);
    }
}
