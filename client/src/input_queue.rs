use protocol::{ActionType, Coord};

#[derive(Debug, Clone)]
pub struct PredictedState {
    pub pos: Coord,
}

#[derive(Debug, Clone)]
pub struct PendingAction {
    pub seq: u32,
    pub action: ActionType,
    pub predicted: Option<PredictedState>,
}

pub struct InputQueue {
    pending: Vec<PendingAction>,
    next_seq: u32,
}

impl InputQueue {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            next_seq: 1,
        }
    }

    pub fn push(&mut self, action: ActionType, predicted: Option<PredictedState>) -> u32 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.pending.push(PendingAction {
            seq,
            action,
            predicted,
        });
        seq
    }

    /// Acknowledge a sequence number. Removes all pending actions with seq <= the given seq.
    /// Returns the removed actions so the caller can compare predictions.
    pub fn ack_up_to(&mut self, seq: u32) -> Vec<PendingAction> {
        let split_point = self
            .pending
            .iter()
            .position(|p| p.seq > seq)
            .unwrap_or(self.pending.len());
        self.pending.drain(..split_point).collect()
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn next_seq(&self) -> u32 {
        self.next_seq
    }
}

impl Default for InputQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_ack() {
        let mut queue = InputQueue::new();
        assert_eq!(queue.push(ActionType::MoveUp, None), 1);
        assert_eq!(queue.push(ActionType::MoveRight, None), 2);
        assert_eq!(queue.pending_count(), 2);

        let acked = queue.ack_up_to(1);
        assert_eq!(acked.len(), 1);
        assert_eq!(acked[0].seq, 1);
        assert_eq!(queue.pending_count(), 1);

        let acked = queue.ack_up_to(2);
        assert_eq!(acked.len(), 1);
        assert_eq!(acked[0].seq, 2);
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn test_ack_beyond() {
        let mut queue = InputQueue::new();
        queue.push(ActionType::MoveUp, None);
        queue.push(ActionType::MoveRight, None);
        queue.push(ActionType::MoveDown, None);

        let acked = queue.ack_up_to(5);
        assert_eq!(acked.len(), 3);
        assert_eq!(queue.pending_count(), 0);
    }
}
