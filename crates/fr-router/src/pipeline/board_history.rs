use fr_board::prelude::Board;
use fr_settings::ScoringSettings;

use crate::score::BoardStatistics;

#[derive(Debug, Clone)]
pub struct BoardHistory {
    boards: Vec<BoardHistoryEntry>,
    max_history_size: usize,
    scoring: ScoringSettings,
}

#[derive(Debug, Clone)]
pub struct BoardHistoryEntry {
    pub board: Board,
    pub hash: u64,
    pub score: f32,
    pub penalty: f64,
    pub restore_count: i32,
}

impl BoardHistoryEntry {
    fn new(board: &mut Board, scoring: &ScoringSettings) -> BoardHistoryEntry {
        let snapshot = board.clone();
        let hash = board.structural_hash();
        let statistics = BoardStatistics::new(board);
        BoardHistoryEntry {
            board: snapshot,
            hash,
            score: statistics.normalized_score(scoring),
            penalty: statistics.routing_penalty(scoring),
            restore_count: 0,
        }
    }
}

impl BoardHistory {
    pub const MAX_HISTORY_SIZE: usize = 30;

    pub fn new(scoring: &ScoringSettings) -> BoardHistory {
        BoardHistory::with_capacity(scoring, BoardHistory::MAX_HISTORY_SIZE)
    }

    pub fn with_capacity(scoring: &ScoringSettings, max_history_size: usize) -> BoardHistory {
        BoardHistory {
            boards: Vec::new(),
            max_history_size,
            scoring: scoring.clone(),
        }
    }

    pub fn add(&mut self, board: &mut Board) {
        if self.contains(board) {
            return;
        }

        if self.boards.len() >= self.max_history_size {
            let new_penalty = BoardStatistics::new(board).routing_penalty(&self.scoring);

            let mut worst_index = 0;
            let mut worst_penalty = self.boards[0].penalty;
            for i in 1..self.boards.len() {
                if self.boards[i].penalty > worst_penalty {
                    worst_penalty = self.boards[i].penalty;
                    worst_index = i;
                }
            }

            if new_penalty >= worst_penalty {
                return;
            }
            self.boards.remove(worst_index);
        }

        let entry = BoardHistoryEntry::new(board, &self.scoring);
        self.boards.push(entry);
        self.boards
            .sort_by(|left, right| left.penalty.total_cmp(&right.penalty));
    }

    pub fn clear(&mut self) {
        self.boards.clear();
    }

    pub fn contains(&self, board: &Board) -> bool {
        let hash = board.structural_hash();
        for entry in &self.boards {
            if entry.hash == hash {
                return true;
            }
        }
        false
    }

    pub fn remove(&mut self, board: &Board) {
        let hash = board.structural_hash();
        for i in 0..self.boards.len() {
            if self.boards[i].hash == hash {
                self.boards.remove(i);
                return;
            }
        }
    }

    pub fn best_penalty(&self) -> f64 {
        self.boards
            .iter()
            .map(|entry| entry.penalty)
            .fold(f64::INFINITY, f64::min)
    }

    pub fn restore_board(&mut self, max_allowed_restore_count: i32) -> Option<Board> {
        let max_allowed_restore_count = if max_allowed_restore_count <= 0 {
            i32::MAX
        } else {
            max_allowed_restore_count
        };

        for entry in &mut self.boards {
            if entry.restore_count <= max_allowed_restore_count {
                entry.restore_count += 1;
                return Some(entry.board.deep_copy());
            }
        }
        None
    }

    pub fn restore_best_board(&mut self) -> Option<Board> {
        self.restore_board(0)
    }

    pub fn size(&self) -> usize {
        self.boards.len()
    }

    pub fn rank(&self, board: &Board) -> i32 {
        let hash = board.structural_hash();
        for (i, entry) in self.boards.iter().enumerate() {
            if entry.hash == hash {
                return i32::try_from(i + 1).unwrap_or(i32::MAX);
            }
        }
        -1
    }

    pub fn entries(&self) -> &[BoardHistoryEntry] {
        &self.boards
    }
}
