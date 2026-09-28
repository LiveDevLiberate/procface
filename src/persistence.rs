//! SQLite 只在显式路径和 feature 同时启用时写入；错误隔离到该 worker。

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::model::{random_id, Kind, Sample, Status};
    fn batch(seq: u64, uptime: f64, payload: usize) -> Arc<SampleBatch> {
        Arc::new(SampleBatch {
            wire: None,
            schema_version: 1,
            session_id: "test-session".into(),
            sequence: seq,
            uptime_s: uptime,
            timestamp_unix: None,
            group: "trace".into(),
            samples: vec![Sample {
                schema_version: 1,
                session_id: "test-session".into(),
                sequence: seq,
                uptime_s: uptime,
                timestamp_unix: None,
                metric: "trace.status".into(),
                entity: "process:1:1".into(),
                value: "x".repeat(payload).into(),
                unit: "text".into(),
                kind: Kind::Gauge,
                status: Status::Ok,
            }],
            processes: vec![],
            complete: true,
            diagnostics: vec![],
        })
    }
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!("procface-db-{}.db", random_id().unwrap()))
    }
    fn send(p: &Persistence, b: Arc<SampleBatch>) {
        // 测试使用阻塞发送以确保每个样本进入队列；生产采集仅使用 try_send。
        p.tx.lock().unwrap().as_ref().unwrap().send(b).unwrap();
    }
    #[test]
    fn shutdown_flushes_pending_and_retention_evicts() {
        let path = path();
        let p = Persistence::start(Some(path.clone()), 65536, 10, 30, "test-boot".into()).unwrap();
        send(&p, batch(1, 1.0, 200));
        send(&p, batch(2, 20.0, 200));
        p.shutdown();
        assert_ne!(p.status.lock().unwrap()["state"], "error");
        let db = rusqlite::Connection::open(&path).unwrap();
        let rows: Vec<u64> = db
            .prepare("SELECT seq FROM samples ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, vec![2]);
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn size_limit_reuses_pages_without_stopping_writer() {
        let path = path();
        let p =
            Persistence::start(Some(path.clone()), 65536, 3600, 30, "test-boot".into()).unwrap();
        for seq in 1..=80 {
            send(&p, batch(seq, seq as f64, 4000));
        }
        p.shutdown();
        assert_ne!(p.status.lock().unwrap()["state"], "error");
        assert!(std::fs::metadata(&path).unwrap().len() <= 65536);
        let db = rusqlite::Connection::open(&path).unwrap();
        let (first, last): (u64, u64) = db
            .query_row("SELECT MIN(seq),MAX(seq) FROM samples", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert!(first > 1);
        assert_eq!(last, 80);
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn oversized_existing_database_is_not_modified() {
        let path = path();
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE existing(data BLOB); INSERT INTO existing VALUES(zeroblob(131072));",
        )
        .unwrap();
        drop(db);
        let before = std::fs::read(&path).unwrap();
        let p =
            Persistence::start(Some(path.clone()), 65536, 3600, 30, "test-boot".into()).unwrap();
        p.shutdown();
        assert_eq!(p.status.lock().unwrap()["state"], "error");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn retention_survives_daemon_and_device_restarts() {
        let path = path();
        for (seq, uptime, boot, expected) in [
            (1, 100.0, "a", vec![1]),
            (2, 105.0, "a", vec![1, 2]),
            (3, 111.0, "a", vec![2, 3]),
            (4, 1.0, "b", vec![2, 3, 4]),
            (5, 12.0, "b", vec![5]),
        ] {
            let p = Persistence::start(Some(path.clone()), 65536, 10, 30, boot.into()).unwrap();
            let mut b = (*batch(seq, uptime, 20)).clone();
            b.session_id = format!("session-{seq}");
            send(&p, Arc::new(b));
            p.shutdown();
            assert_ne!(p.status.lock().unwrap()["state"], "error");
            let db = rusqlite::Connection::open(&path).unwrap();
            let rows = db
                .prepare("SELECT seq FROM samples ORDER BY id")
                .unwrap()
                .query_map([], |r| r.get::<_, u64>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(rows, expected);
        }
        std::fs::remove_file(path).unwrap();
    }
}

use crate::model::SampleBatch;
#[cfg(feature = "sqlite")]
use crate::store::Store;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        mpsc::{SyncSender, TrySendError},
        Arc, Mutex,
    },
};
#[cfg(feature = "sqlite")]
use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
pub struct Persistence {
    tx: Mutex<Option<SyncSender<Arc<SampleBatch>>>>,
    worker: Mutex<Option<std::thread::JoinHandle<()>>>,
    pub status: Arc<Mutex<Value>>,
}
impl Persistence {
    pub fn disabled() -> Self {
        Self {
            tx: Mutex::new(None),
            worker: Mutex::new(None),
            status: Arc::new(Mutex::new(json!({"state":"disabled"}))),
        }
    }
    pub fn start(
        path: Option<PathBuf>,
        max_bytes: u64,
        retention_seconds: u64,
        flush_seconds: u64,
        boot_id: String,
    ) -> Result<Self, String> {
        let Some(path) = path else {
            return Ok(Self::disabled());
        };
        #[cfg(not(feature = "sqlite"))]
        {
            let _ = (path, max_bytes, retention_seconds, flush_seconds, boot_id);
            Err("当前构建不含 SQLite；请使用 diagnostic 构建".into())
        }
        #[cfg(feature = "sqlite")]
        {
            if !(65536..=1024 * 1024 * 1024).contains(&max_bytes)
                || retention_seconds == 0
                || flush_seconds == 0
            {
                return Err("SQLite 大小须为 64 KiB 到 1 GiB，保留与 flush 间隔须大于零".into());
            }
            let (tx, rx) = mpsc::sync_channel::<Arc<SampleBatch>>(1);
            let status = Arc::new(Mutex::new(json!({"state":"starting"})));
            let worker_status = status.clone();
            let worker = thread::spawn(move || {
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    use rusqlite::OptionalExtension;
                    let mut connection = rusqlite::Connection::open(&path)?;
                    connection.busy_timeout(Duration::from_millis(500))?;
                    let page_size: u64 =
                        connection.query_row("PRAGMA page_size", [], |r| r.get(0))?;
                    let page_count: u64 =
                        connection.query_row("PRAGMA page_count", [], |r| r.get(0))?;
                    let max_pages = max_bytes / page_size;
                    if page_count > max_pages {
                        return Err("已有数据库大于配置上限，拒绝写入且保留原文件".into());
                    }
                    connection.pragma_update(None, "max_page_count", max_pages)?;
                    connection.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA synchronous=NORMAL; CREATE TABLE IF NOT EXISTS samples(id INTEGER PRIMARY KEY, session TEXT NOT NULL, seq INTEGER NOT NULL, uptime REAL NOT NULL, payload TEXT NOT NULL, bytes INTEGER NOT NULL);")?;
                    let has_clock = connection
                        .prepare("PRAGMA table_info(samples)")?
                        .query_map([], |r| r.get::<_, String>(1))?
                        .collect::<Result<Vec<_>, _>>()?
                        .iter()
                        .any(|name| name == "retention_time");
                    if !has_clock {
                        connection.execute_batch("ALTER TABLE samples ADD COLUMN retention_time REAL NOT NULL DEFAULT 0;")?;
                    }
                    connection.execute_batch("CREATE TABLE IF NOT EXISTS retention_clock(id INTEGER PRIMARY KEY CHECK(id=1), boot TEXT NOT NULL, uptime REAL NOT NULL, elapsed REAL NOT NULL);")?;
                    let saved_clock: Option<(String, f64, f64)> = connection
                        .query_row(
                            "SELECT boot,uptime,elapsed FROM retention_clock WHERE id=1",
                            [],
                            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                        )
                        .optional()?;
                    let (mut clock_boot, mut clock_uptime, mut elapsed) =
                        saved_clock.unwrap_or_default();
                    *worker_status.lock().unwrap() = json!({"state":"enabled","max_bytes":max_bytes,"retention_seconds":retention_seconds,"flush_seconds":flush_seconds,"retention_clock":"observed_uptime"});
                    let mut pending = Vec::new();
                    let mut pending_bytes = 0;
                    let mut last = Instant::now();
                    let mut dropped = 0u64;
                    loop {
                        let result = rx.recv_timeout(
                            Duration::from_secs(flush_seconds).saturating_sub(last.elapsed()),
                        );
                        let disconnected =
                            matches!(result, Err(mpsc::RecvTimeoutError::Disconnected));
                        if let Ok(b) = result {
                            let bytes = Store::estimate(&b);
                            if pending_bytes + bytes <= 1024 * 1024 {
                                pending_bytes += bytes;
                                pending.push(b);
                            } else {
                                dropped += 1;
                            }
                        }
                        if last.elapsed() >= Duration::from_secs(flush_seconds) || disconnected {
                            if !pending.is_empty() {
                                let tx = connection.transaction()?;
                                for b in &pending {
                                    if clock_boot != boot_id {
                                        clock_boot.clone_from(&boot_id);
                                        clock_uptime = b.uptime_s;
                                    } else if b.uptime_s > clock_uptime {
                                        elapsed += b.uptime_s - clock_uptime;
                                        clock_uptime = b.uptime_s;
                                    }
                                    let retention_time =
                                        elapsed - (clock_uptime - b.uptime_s).max(0.0);
                                    tx.execute(
                                        "DELETE FROM samples WHERE retention_time<?1",
                                        [elapsed - retention_seconds as f64],
                                    )?;
                                    let payload = serde_json::to_string(&**b)?;
                                    let bytes = payload.len() as u64;
                                    if bytes > max_bytes / 4
                                        || retention_time < elapsed - retention_seconds as f64
                                    {
                                        dropped += 1;
                                        continue;
                                    }
                                    // 留出索引、页与事务余量，DELETE 后 SQLite 复用空闲页。
                                    loop {
                                        let used: u64 = tx.query_row(
                                            "SELECT COALESCE(SUM(bytes),0) FROM samples",
                                            [],
                                            |r| r.get(0),
                                        )?;
                                        if used + bytes <= max_bytes / 2 {
                                            break;
                                        }
                                        if tx.execute("DELETE FROM samples WHERE id=(SELECT MIN(id) FROM samples)",[])?==0{break;}
                                    }
                                    tx.execute("INSERT INTO samples(session,seq,uptime,payload,bytes,retention_time) VALUES(?1,?2,?3,?4,?5,?6)",rusqlite::params![b.session_id,b.sequence,b.uptime_s,payload,bytes,retention_time])?;
                                }
                                tx.execute("INSERT OR REPLACE INTO retention_clock(id,boot,uptime,elapsed) VALUES(1,?1,?2,?3)", rusqlite::params![clock_boot,clock_uptime,elapsed])?;
                                tx.commit()?;
                                pending.clear();
                                pending_bytes = 0;
                            }
                            last = Instant::now();
                            worker_status.lock().unwrap()["dropped_batches"] = json!(dropped);
                        }
                        if disconnected {
                            break;
                        }
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    eprintln!("SQLite 持久化已停止: {e}");
                    *worker_status.lock().unwrap() = json!({"state":"error","error":e.to_string()});
                }
            });
            Ok(Self {
                tx: Mutex::new(Some(tx)),
                worker: Mutex::new(Some(worker)),
                status,
            })
        }
    }
    pub fn offer(&self, b: Arc<SampleBatch>) {
        if let Some(tx) = &*self.tx.lock().unwrap() {
            match tx.try_send(b) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    let mut state = self.status.lock().unwrap();
                    let n = state["queue_dropped"].as_u64().unwrap_or(0);
                    state["queue_dropped"] = json!(n + 1);
                }
                Err(TrySendError::Disconnected(_)) => {}
            }
        }
    }
    /// 停止接收后排空有界队列，提交剩余事务再返回。
    pub fn shutdown(&self) {
        self.tx.lock().unwrap().take();
        if let Some(worker) = self.worker.lock().unwrap().take() {
            if worker.join().is_err() {
                eprintln!("SQLite worker 异常退出");
                *self.status.lock().unwrap() = json!({"state":"error","error":"worker_panicked"});
            }
        }
    }
}
