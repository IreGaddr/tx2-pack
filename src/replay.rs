use crate::error::{PackError, Result};
use crate::format::PackedSnapshot;
use crate::checkpoint::{Checkpoint, CheckpointManager};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayDirection {
    Forward,
    Backward,
}

pub struct ReplayEngine {
    checkpoints: VecDeque<Checkpoint>,
    current_index: usize,
    loop_replay: bool,
}

impl ReplayEngine {
    pub fn new() -> Self {
        Self {
            checkpoints: VecDeque::new(),
            current_index: 0,
            loop_replay: false,
        }
    }

    pub fn with_loop(mut self, enabled: bool) -> Self {
        self.loop_replay = enabled;
        self
    }

    pub fn add_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.checkpoints.push_back(checkpoint);
    }

    pub fn load_from_manager(&mut self, manager: &mut CheckpointManager) -> Result<()> {
        self.checkpoints.clear();

        let chain = manager.get_checkpoint_chain().to_vec();
        for id in chain {
            let checkpoint = manager.load_checkpoint(&id)?;
            self.checkpoints.push_back(checkpoint);
        }

        self.current_index = 0;

        Ok(())
    }

    pub fn current(&self) -> Option<&Checkpoint> {
        self.checkpoints.get(self.current_index)
    }

    pub fn next(&mut self) -> Option<&Checkpoint> {
        if self.current_index + 1 < self.checkpoints.len() {
            self.current_index += 1;
            self.current()
        } else if self.loop_replay && !self.checkpoints.is_empty() {
            self.current_index = 0;
            self.current()
        } else {
            None
        }
    }

    pub fn previous(&mut self) -> Option<&Checkpoint> {
        if self.current_index > 0 {
            self.current_index -= 1;
            self.current()
        } else if self.loop_replay && !self.checkpoints.is_empty() {
            self.current_index = self.checkpoints.len() - 1;
            self.current()
        } else {
            None
        }
    }

    pub fn seek(&mut self, index: usize) -> Result<&Checkpoint> {
        if index >= self.checkpoints.len() {
            return Err(PackError::InvalidCheckpoint(
                format!("Index {} out of bounds", index)
            ));
        }

        self.current_index = index;
        self.current()
            .ok_or_else(|| PackError::InvalidCheckpoint("No checkpoint at index".to_string()))
    }

    pub fn seek_to_start(&mut self) -> Option<&Checkpoint> {
        self.current_index = 0;
        self.current()
    }

    pub fn seek_to_end(&mut self) -> Option<&Checkpoint> {
        if !self.checkpoints.is_empty() {
            self.current_index = self.checkpoints.len() - 1;
        }
        self.current()
    }

    pub fn get_index(&self) -> usize {
        self.current_index
    }

    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }

    pub fn is_at_start(&self) -> bool {
        self.current_index == 0
    }

    pub fn is_at_end(&self) -> bool {
        self.current_index == self.checkpoints.len().saturating_sub(1)
    }

    pub fn clear(&mut self) {
        self.checkpoints.clear();
        self.current_index = 0;
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TimeTravel {
    snapshots: Vec<(f64, PackedSnapshot)>,
    current_time: f64,
}

impl TimeTravel {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            current_time: 0.0,
        }
    }

    pub fn record(&mut self, time: f64, snapshot: PackedSnapshot) {
        self.snapshots.push((time, snapshot));
        self.snapshots.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        self.current_time = time;
    }

    pub fn seek_to_time(&mut self, target_time: f64) -> Option<&PackedSnapshot> {
        let index = self.find_snapshot_at_time(target_time)?;
        self.current_time = self.snapshots[index].0;
        Some(&self.snapshots[index].1)
    }

    pub fn get_snapshot_at_time(&self, time: f64) -> Option<&PackedSnapshot> {
        let index = self.find_snapshot_at_time(time)?;
        Some(&self.snapshots[index].1)
    }

    pub fn get_current_snapshot(&self) -> Option<&PackedSnapshot> {
        self.get_snapshot_at_time(self.current_time)
    }

    pub fn get_earliest_time(&self) -> Option<f64> {
        self.snapshots.first().map(|(t, _)| *t)
    }

    pub fn get_latest_time(&self) -> Option<f64> {
        self.snapshots.last().map(|(t, _)| *t)
    }

    pub fn get_current_time(&self) -> f64 {
        self.current_time
    }

    pub fn fork_at_time(&self, time: f64) -> Option<PackedSnapshot> {
        self.get_snapshot_at_time(time).cloned()
    }

    pub fn prune_before(&mut self, time: f64) {
        self.snapshots.retain(|(t, _)| *t >= time);
    }

    pub fn prune_after(&mut self, time: f64) {
        self.snapshots.retain(|(t, _)| *t <= time);
    }

    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.current_time = 0.0;
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    fn find_snapshot_at_time(&self, target_time: f64) -> Option<usize> {
        if self.snapshots.is_empty() {
            return None;
        }

        let mut left = 0;
        let mut right = self.snapshots.len();

        while left < right {
            let mid = (left + right) / 2;
            if self.snapshots[mid].0 < target_time {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left > 0 && (left >= self.snapshots.len() ||
           (self.snapshots[left].0 - target_time).abs() >
           (target_time - self.snapshots[left - 1].0).abs()) {
            Some(left - 1)
        } else if left < self.snapshots.len() {
            Some(left)
        } else if left > 0 {
            Some(left - 1)
        } else {
            None
        }
    }
}

impl Default for TimeTravel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_engine() {
        let mut engine = ReplayEngine::new();

        for i in 0..5 {
            let checkpoint = Checkpoint::new(format!("cp{}", i), PackedSnapshot::new());
            engine.add_checkpoint(checkpoint);
        }

        assert_eq!(engine.len(), 5);
        assert_eq!(engine.get_index(), 0);

        engine.next();
        assert_eq!(engine.get_index(), 1);

        engine.previous();
        assert_eq!(engine.get_index(), 0);

        engine.seek_to_end();
        assert_eq!(engine.get_index(), 4);
        assert!(engine.is_at_end());

        engine.seek_to_start();
        assert_eq!(engine.get_index(), 0);
        assert!(engine.is_at_start());
    }

    #[test]
    fn test_replay_loop() {
        let mut engine = ReplayEngine::new().with_loop(true);

        for i in 0..3 {
            let checkpoint = Checkpoint::new(format!("cp{}", i), PackedSnapshot::new());
            engine.add_checkpoint(checkpoint);
        }

        engine.seek_to_end();
        assert_eq!(engine.get_index(), 2);

        engine.next();
        assert_eq!(engine.get_index(), 0);

        engine.seek_to_start();
        assert_eq!(engine.get_index(), 0);

        engine.previous();
        assert_eq!(engine.get_index(), 2);
    }

    #[test]
    fn test_time_travel() {
        let mut tt = TimeTravel::new();

        for i in 0..10 {
            let snapshot = PackedSnapshot::new();
            tt.record(i as f64 * 10.0, snapshot);
        }

        assert_eq!(tt.len(), 10);
        assert_eq!(tt.get_earliest_time(), Some(0.0));
        assert_eq!(tt.get_latest_time(), Some(90.0));

        let snapshot = tt.seek_to_time(45.0);
        assert!(snapshot.is_some());
        assert_eq!(tt.get_current_time(), 50.0);

        tt.prune_before(30.0);
        assert_eq!(tt.len(), 7);
        assert_eq!(tt.get_earliest_time(), Some(30.0));

        tt.prune_after(70.0);
        assert_eq!(tt.len(), 5);
        assert_eq!(tt.get_latest_time(), Some(70.0));
    }

    #[test]
    fn test_time_travel_fork() {
        let mut tt = TimeTravel::new();

        for i in 0..5 {
            let snapshot = PackedSnapshot::new();
            tt.record(i as f64 * 10.0, snapshot);
        }

        let forked = tt.fork_at_time(20.0);
        assert!(forked.is_some());
    }
}
