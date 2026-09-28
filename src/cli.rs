use crate::{
    collector::{self, Collector, DEFAULT_GROUPS, GROUPS},
    model::{self, Sample, SampleBatch},
    trace::{TraceOptions, Tracer},
};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Parser, Debug)]
#[command(name="procface",version,about="嵌入式 Linux procfs 性能分析",after_help=feature_help())]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand, Debug)]
pub enum Command {
    /// 本地系统采样；默认持续输出，--count 指定有限轮数
    Sample(SampleArgs),
    /// 单 PID 深度追踪（不依赖 daemon）
    Trace(TraceArgs),
    /// 输出当前内核与构建能力
    Capabilities {
        #[arg(long, default_value = "/proc")]
        proc_root: PathBuf,
    },
    /// 启动仅提供 API 的 daemon
    Daemon(crate::daemon::DaemonArgs),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Table,
    Tsv,
    Json,
    Jsonl,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Timestamp {
    Unix,
}
#[derive(Args, Clone, Debug)]
pub struct OutputArgs {
    #[arg(long,default_value="1",value_parser=parse_interval)]
    pub interval: f64,
    #[arg(long, default_value = "0")]
    pub count: u64,
    #[arg(long, value_enum, default_value = "table")]
    pub format: Format,
    #[arg(long, value_enum)]
    pub timestamp: Option<Timestamp>,
    #[arg(long, default_value = "/proc")]
    pub proc_root: PathBuf,
    /// 仅 table 输出使用 KiB、MiB 等单位
    #[arg(long)]
    pub human: bool,
    /// 仅 table 输出启用 ANSI 颜色，默认关闭
    #[arg(long)]
    pub colors: bool,
}
#[derive(Args, Debug)]
pub struct SampleArgs {
    #[command(flatten)]
    pub output: OutputArgs,
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "cpu,memory,load,time,disk,network,vm"
    )]
    pub metrics: Vec<String>,
    #[arg(long, default_value = "self")]
    pub user: String,
    #[arg(long, value_delimiter = ',')]
    pub pids: Vec<u32>,
}
#[derive(Args, Debug)]
pub struct TraceArgs {
    pub pid: u32,
    #[command(flatten)]
    pub output: OutputArgs,
    #[arg(long, default_value = "self")]
    pub user: String,
    /// 仅采集基础字段，不读取中等成本的诊断文件
    #[arg(long)]
    pub basic: bool,
    #[arg(long)]
    pub threads: bool,
    #[arg(long)]
    pub diagnostic_sensitive: bool,
    #[arg(long,default_value="500",value_parser=clap::value_parser!(u64).range(1..))]
    pub budget_ms: u64,
}
pub fn parse_interval(s: &str) -> Result<f64, String> {
    let n = s.parse::<f64>().map_err(|_| "间隔必须是秒数")?;
    if n.is_finite() && (1.0..=86400.0).contains(&n) {
        Ok(n)
    } else {
        Err("间隔必须在 1 到 86400 秒之间".into())
    }
}
pub fn feature_help() -> String {
    format!("特性：procfs、CLI、trace、daemon、HTTP Bearer 鉴权；前端独立发布\nSQLite：{}；敏感诊断：{}（均默认不启用）\n默认：1 秒采样、当前用户进程范围、daemon 127.0.0.1、拒绝跨域、无颜色\nstdout 仅输出数据；诊断写 stderr。JSON 必须指定有限 --count，持续输出可用 JSONL。",if cfg!(feature="sqlite"){"已编译"}else{"未编译"},if cfg!(feature="diagnostic"){"已编译"}else{"未编译"})
}
pub fn selected_groups(input: &[String]) -> io::Result<Vec<String>> {
    if input.is_empty() {
        return Ok(DEFAULT_GROUPS.iter().map(|v| v.to_string()).collect());
    }
    if let Some(v) = input.iter().find(|v| !GROUPS.contains(&v.as_str())) {
        return Err(io::Error::other(format!("未知指标组: {v}")));
    }
    Ok(input.to_vec())
}
fn clean(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
fn table_value(s: &Sample, human: bool) -> String {
    if s.value.is_null() {
        return "N/A".into();
    }
    if let Some(n) = s.value.as_f64() {
        if human && s.unit == "bytes" {
            for (scale, unit) in [(1073741824.0, "GiB"), (1048576.0, "MiB"), (1024.0, "KiB")] {
                if n.abs() >= scale {
                    return format!("{:.3} {unit}", n / scale);
                }
            }
        }
        if s.value.is_f64() {
            return format!("{n:.6}");
        }
    }
    match &s.value {
        serde_json::Value::String(v) => clean(v),
        v => v.to_string(),
    }
}
pub struct Output<W: Write> {
    writer: W,
    format: Format,
    human: bool,
    colors: bool,
    first: bool,
}
impl<W: Write> Output<W> {
    pub fn new(mut writer: W, args: &OutputArgs) -> io::Result<Self> {
        if args.format == Format::Json && args.count == 0 {
            return Err(io::Error::other(
                "JSON 文档必须指定 --count N；持续输出请使用 --format jsonl",
            ));
        }
        match args.format {
            Format::Json => write!(writer, "["),
            Format::Tsv => writeln!(writer, "{}", model::TSV_HEADER),
            Format::Table => writeln!(writer, "UPTIME(s)\tENTITY\tMETRIC\tVALUE\tUNIT\tSTATUS"),
            Format::Jsonl => Ok(()),
        }?;
        Ok(Self {
            writer,
            format: args.format,
            human: args.human,
            colors: args.colors,
            first: true,
        })
    }
    pub fn batch(&mut self, b: &SampleBatch) -> io::Result<()> {
        for message in &b.diagnostics {
            eprintln!("{message}");
        }
        for s in &b.samples {
            match self.format {
                Format::Json => {
                    if !self.first {
                        write!(self.writer, ",")?;
                    }
                    serde_json::to_writer(&mut self.writer, s)?;
                }
                Format::Jsonl => {
                    serde_json::to_writer(&mut self.writer, s)?;
                    writeln!(self.writer)?;
                }
                Format::Tsv => model::write_tsv(&mut self.writer, s)?,
                Format::Table => {
                    let value = table_value(s, self.human);
                    let status = serde_json::to_value(s.status).unwrap();
                    let prefix = if self.colors { "\x1b[36m" } else { "" };
                    let suffix = if self.colors { "\x1b[0m" } else { "" };
                    writeln!(
                        self.writer,
                        "{prefix}{:.3}\t{}\t{}\t{}\t{}\t{}{suffix}",
                        s.uptime_s,
                        clean(&s.entity),
                        s.metric,
                        value,
                        s.unit,
                        status.as_str().unwrap()
                    )?;
                }
            }
            self.first = false;
        }
        self.writer.flush()
    }
    pub fn finish(&mut self) -> io::Result<()> {
        if self.format == Format::Json {
            writeln!(self.writer, "]")?;
        }
        self.writer.flush()
    }
}
fn run_samples(
    args: &OutputArgs,
    mut next: impl FnMut() -> io::Result<Vec<SampleBatch>>,
) -> io::Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let main_thread = std::thread::current();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        main_thread.unpark();
    })
    .map_err(io::Error::other)?;
    let stdout = io::stdout();
    let mut out = Output::new(stdout.lock(), args)?;
    let interval = Duration::from_secs_f64(args.interval);
    let mut count = 0;
    let mut result = Ok(());
    while running.load(Ordering::SeqCst) && (args.count == 0 || count < args.count) {
        let deadline = Instant::now() + interval;
        match next() {
            Ok(batches) => {
                for b in batches {
                    if let Err(e) = out.batch(&b) {
                        result = Err(e);
                        break;
                    }
                }
            }
            Err(e) => {
                result = Err(e);
                break;
            }
        }
        if result.is_err() {
            break;
        }
        count += 1;
        if args.count != 0 && count >= args.count {
            break;
        }
        while running.load(Ordering::SeqCst) && Instant::now() < deadline {
            std::thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
        }
    }
    out.finish()?;
    result
}
pub fn run(cli: Cli) -> io::Result<()> {
    match cli.command {
        Command::Capabilities { proc_root } => {
            serde_json::to_writer_pretty(
                io::stdout().lock(),
                &collector::capabilities_document(&proc_root),
            )?;
            println!();
            Ok(())
        }
        Command::Sample(args) => {
            let groups = selected_groups(&args.metrics)?;
            let mut c = Collector::new(args.output.proc_root.clone())?;
            c.uid = collector::user_id(&args.user).map_err(io::Error::other)?;
            c.pids = args.pids;
            c.timestamp = args.output.timestamp.is_some();
            run_samples(&args.output, || {
                let mut b = vec![c.system(&groups)?];
                if groups.iter().any(|g| g == "process") {
                    b.push(c.processes()?);
                }
                Ok(b)
            })
        }
        Command::Trace(args) => {
            let uid = collector::user_id(&args.user).map_err(io::Error::other)?;
            let options = TraceOptions {
                pid: args.pid,
                extended: !args.basic,
                threads: args.threads,
                sensitive: args.diagnostic_sensitive,
                budget_ms: args.budget_ms,
            };
            let mut t = Tracer::new(args.output.proc_root.clone(), options, uid)?;
            t.collector.timestamp = args.output.timestamp.is_some();
            run_samples(&args.output, || Ok(vec![t.sample()?]))
        }
        Command::Daemon(args) => crate::daemon::run(args),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_contract() {
        let cli = Cli::try_parse_from(["procface", "daemon"]).unwrap();
        let Command::Daemon(daemon) = cli.command else {
            panic!("daemon 子命令解析错误")
        };
        assert_eq!(daemon.listen, "127.0.0.1:9387".parse().unwrap());
        assert!(daemon.token.is_none());
        assert!(daemon.allow_origin.is_empty());
        assert!(!daemon.allow_file_origin && !daemon.allow_unsigned_frontend);
        assert_eq!(daemon.history_seconds, 60);
        assert_eq!(daemon.process_budget_ms, 500);
        assert!(daemon.sqlite_path.is_none());
        assert_eq!(daemon.sqlite_max_bytes, 16 * 1024 * 1024);
        assert_eq!(daemon.sqlite_retention_seconds, 3600);
        assert_eq!(daemon.sqlite_flush_seconds, 30);
        assert!(Cli::try_parse_from(["procface", "sample", "--interval", "0.9"]).is_err());
        assert!(Cli::try_parse_from(["procface", "sample", "--interval", "NaN"]).is_err());
        assert!(Cli::try_parse_from(["procface", "sample", "--unknown"]).is_err());
        let args = OutputArgs {
            interval: 1.0,
            count: 0,
            format: Format::Json,
            timestamp: None,
            proc_root: "/proc".into(),
            human: false,
            colors: false,
        };
        assert!(Output::new(Vec::new(), &args).is_err());
    }
}
