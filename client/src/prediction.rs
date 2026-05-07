use protocol::{ActionType, Coord};

/// Predict the result of a move action. Returns the new position.
pub fn predict_move(current: Coord, action: &ActionType) -> Coord {
    match action {
        ActionType::MoveUp => Coord::new(current.x, current.y - 1),
        ActionType::MoveDown => Coord::new(current.x, current.y + 1),
        ActionType::MoveLeft => Coord::new(current.x - 1, current.y),
        ActionType::MoveRight => Coord::new(current.x + 1, current.y),
        ActionType::MoveUpLeft => Coord::new(current.x - 1, current.y - 1),
        ActionType::MoveUpRight => Coord::new(current.x + 1, current.y - 1),
        ActionType::MoveDownLeft => Coord::new(current.x - 1, current.y + 1),
        ActionType::MoveDownRight => Coord::new(current.x + 1, current.y + 1),
        _ => current,
    }
}

/// Check if a rollback is needed. Returns the corrected position if the
/// prediction was wrong, or None if the prediction matches the confirmed position.
pub fn check_rollback(predicted: Coord, confirmed: Coord) -> Option<Coord> {
    if predicted != confirmed {
        Some(confirmed)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predict_move() {
        let pos = Coord::new(10, 10);
        assert_eq!(predict_move(pos, &ActionType::MoveUp), Coord::new(10, 9));
        assert_eq!(
            predict_move(pos, &ActionType::MoveDown),
            Coord::new(10, 11)
        );
        assert_eq!(
            predict_move(pos, &ActionType::MoveLeft),
            Coord::new(9, 10)
        );
        assert_eq!(
            predict_move(pos, &ActionType::MoveRight),
            Coord::new(11, 10)
        );
        assert_eq!(
            predict_move(pos, &ActionType::MoveUpLeft),
            Coord::new(9, 9)
        );
        assert_eq!(
            predict_move(pos, &ActionType::MoveUpRight),
            Coord::new(11, 9)
        );
        assert_eq!(
            predict_move(pos, &ActionType::MoveDownLeft),
            Coord::new(9, 11)
        );
        assert_eq!(
            predict_move(pos, &ActionType::MoveDownRight),
            Coord::new(11, 11)
        );
    }

    #[test]
    fn test_predict_wait() {
        let pos = Coord::new(5, 5);
        assert_eq!(predict_move(pos, &ActionType::Wait), pos);
    }

    #[test]
    fn test_rollback_match() {
        let predicted = Coord::new(10, 9);
        let confirmed = Coord::new(10, 9);
        assert_eq!(check_rollback(predicted, confirmed), None);
    }

    #[test]
    fn test_rollback_mismatch() {
        let predicted = Coord::new(10, 9);
        let confirmed = Coord::new(10, 10); // server rejected the move
        assert_eq!(check_rollback(predicted, confirmed), Some(confirmed));
    }
}
