use protocol::{Coord, LogEntry};

pub struct MessageLog {
    entries: Vec<LogEntry>,
    max_entries: usize,
}

impl MessageLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn add(&mut self, text: impl Into<String>, color: u32, turn: u64) {
        self.entries.push(LogEntry {
            text: text.into(),
            color,
            turn,
        });
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    pub fn combat(&mut self, turn: u64, attacker: &str, defender: &str, damage: i32, critical: bool) {
        if damage == 0 {
            self.add(
                format!("{attacker} misses {defender}!"),
                0x888888,
                turn,
            );
        } else if critical {
            self.add(
                format!("{attacker} critically hits {defender} for {damage} damage!"),
                0xFF4444,
                turn,
            );
        } else {
            self.add(
                format!("{attacker} hits {defender} for {damage} damage."),
                0xFF8888,
                turn,
            );
        }
    }

    pub fn movement(&mut self, turn: u64, entity: &str, from: Coord, to: Coord) {
        self.add(
            format!("{entity} moves from ({},{}) to ({},{})", from.x, from.y, to.x, to.y),
            0x8888FF,
            turn,
        );
    }

    pub fn death(&mut self, turn: u64, entity: &str) {
        self.add(
            format!("{entity} has been slain!"),
            0xFF0000,
            turn,
        );
    }

    pub fn info(&mut self, turn: u64, text: impl Into<String>) {
        self.add(text, 0xFFFFFF, turn);
    }

    pub fn recent(&self, count: usize) -> Vec<LogEntry> {
        self.entries
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect()
    }

    pub fn all(&self) -> &[LogEntry] {
        &self.entries
    }
}

impl Default for MessageLog {
    fn default() -> Self {
        Self::new(200)
    }
}
