use crate::model::SampleBatch;
use std::{collections::VecDeque, sync::Arc};

pub struct Store {
    pub history: VecDeque<(Arc<SampleBatch>, usize)>,
    pub bytes: usize,
    pub max_bytes: usize,
    pub window: f64,
    pub dropped: u64,
    pub lost_through: u64,
}
impl Store {
    pub fn new(max_bytes: usize, window: f64) -> Self {
        Self {
            history: VecDeque::new(),
            bytes: 0,
            max_bytes,
            window,
            dropped: 0,
            lost_through: 0,
        }
    }
    pub fn estimate(b: &SampleBatch) -> usize {
        std::mem::size_of::<SampleBatch>()
            + b.wire.as_ref().map_or(0, |wire| wire.estimated_bytes())
            + b.session_id.capacity()
            + b.group.capacity()
            + b.samples.capacity() * std::mem::size_of::<crate::model::Sample>()
            + b.processes.capacity() * std::mem::size_of::<crate::model::ProcessInfo>()
            + b.diagnostics.capacity() * std::mem::size_of::<String>()
            + b.samples
                .iter()
                .map(|s| {
                    s.session_id.capacity()
                        + s.metric.capacity()
                        + s.entity.capacity()
                        + s.unit.capacity()
                        + crate::compact::value_heap_bytes(&s.value)
                })
                .sum::<usize>()
            + b.processes
                .iter()
                .map(|p| p.name.capacity() + p.state.capacity())
                .sum::<usize>()
            + b.diagnostics.iter().map(|s| s.capacity()).sum::<usize>()
    }
    pub fn push(&mut self, b: Arc<SampleBatch>) -> bool {
        let size = Self::estimate(&b);
        if size > self.max_bytes {
            self.dropped += 1;
            self.lost_through = self.lost_through.max(b.sequence);
            return false;
        }
        if self
            .history
            .back()
            .is_some_and(|(old, _)| b.uptime_s < old.uptime_s)
        {
            self.lost_through = self.lost_through.max(
                self.history
                    .iter()
                    .map(|(b, _)| b.sequence)
                    .max()
                    .unwrap_or(0),
            );
            self.history.clear();
            self.bytes = 0;
        }
        while self
            .history
            .front()
            .is_some_and(|(old, _)| b.uptime_s - old.uptime_s > self.window)
            || self.bytes + size > self.max_bytes
        {
            let Some((old, n)) = self.history.pop_front() else {
                break;
            };
            self.bytes -= n;
            self.lost_through = self.lost_through.max(old.sequence);
        }
        self.bytes += size;
        self.history.push_back((b, size));
        true
    }
    /// 即使当前组停止更新，也由主采样时钟淘汰过期历史。
    pub fn expire(&mut self, uptime: f64) {
        while self
            .history
            .front()
            .is_some_and(|(b, _)| uptime - b.uptime_s > self.window)
        {
            let (old, bytes) = self.history.pop_front().unwrap();
            self.bytes -= bytes;
            self.lost_through = self.lost_through.max(old.sequence);
        }
    }
    pub fn latest(&self) -> Vec<Arc<SampleBatch>> {
        let mut groups = std::collections::HashSet::new();
        self.history
            .iter()
            .rev()
            .filter_map(|(b, _)| {
                if groups.insert(&b.group) {
                    Some(b.clone())
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn get(&self, sequence: u64) -> Option<Arc<SampleBatch>> {
        self.history
            .iter()
            .rev()
            .find(|(b, _)| b.sequence == sequence)
            .map(|(b, _)| b.clone())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::random_id;
    fn b(seq: u64, up: f64) -> Arc<SampleBatch> {
        Arc::new(SampleBatch {
            wire: None,
            schema_version: 1,
            session_id: random_id().unwrap(),
            sequence: seq,
            uptime_s: up,
            timestamp_unix: None,
            group: "system".into(),
            samples: vec![],
            processes: vec![],
            complete: true,
            diagnostics: vec![],
        })
    }
    #[test]
    fn allocated_capacity_counts_toward_budget() {
        let mut batch = (*b(1, 1.0)).clone();
        batch.samples.reserve(1024);
        assert!(Store::estimate(&batch) >= 1024 * std::mem::size_of::<crate::model::Sample>());
        assert!(!Store::new(1000, 60.0).push(Arc::new(batch)));
    }
    #[test]
    fn stopped_group_expires_and_middle_gaps_remain_visible() {
        let mut store = Store::new(1000, 2.0);
        store.push(b(2, 1.0));
        store.expire(4.0);
        assert!(store.history.is_empty());
        assert_eq!(store.bytes, 0);
        assert_eq!(store.lost_through, 2);
        let mut oversized = (*b(4, 4.0)).clone();
        oversized.diagnostics.push("x".repeat(2000));
        assert!(!store.push(Arc::new(oversized)));
        store.push(b(5, 4.0));
        assert_eq!(store.lost_through, 4);
        assert_eq!(store.get(5).unwrap().sequence, 5);
    }
    #[test]
    fn evicts_by_time_and_bytes() {
        let mut s = Store::new(1000, 2.0);
        s.push(b(1, 1.0));
        s.push(b(2, 4.0));
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.history[0].0.sequence, 2);
        for i in 3..100 {
            s.push(b(i, 4.0));
            assert!(s.bytes <= 1000);
        }
        let mut tiny = Store::new(1, 60.0);
        assert!(!tiny.push(b(1, 1.0)));
        assert_eq!(tiny.dropped, 1);
    }
}
