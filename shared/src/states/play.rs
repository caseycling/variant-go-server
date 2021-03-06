mod n_plus_one;

use crate::game::{
    find_groups, ActionChange, ActionKind, Board, BoardHistory, Color, GameState, Group, GroupVec,
    MakeActionError, MakeActionResult, Point, Seat, SharedState, VisibilityBoard,
};
use serde::{Deserialize, Serialize};

use bitmaps::Bitmap;
use tinyvec::tiny_vec;

type Revealed = bool;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayState {
    // TODO: use smallvec?
    pub players_passed: Vec<bool>,
    pub last_stone: Option<GroupVec<(u32, u32)>>,
}

impl PlayState {
    pub fn new(seat_count: usize) -> Self {
        PlayState {
            players_passed: vec![false; seat_count],
            last_stone: None,
        }
    }

    fn place_stone(
        &mut self,
        shared: &mut SharedState,
        (x, y): Point,
    ) -> MakeActionResult<GroupVec<Point>> {
        let active_seat = get_active_seat(shared);
        let mut points_played = GroupVec::new();

        if shared.mods.pixel {
            // In pixel mode coordinate 0,0 is outside the board.
            // This is to adjust for it.

            if x > shared.board.width || y > shared.board.height {
                return Err(MakeActionError::OutOfBounds);
            }
            let x = x as i32 - 1;
            let y = y as i32 - 1;

            let mut any_placed = false;
            let mut any_revealed = false;
            for &(x, y) in &[(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)] {
                if x < 0 || y < 0 {
                    continue;
                }
                let coord = (x as u32, y as u32);
                if !shared.board.point_within(coord) {
                    continue;
                }

                let point = shared.board.point_mut(coord);
                if let Some(visibility) = &mut shared.board_visibility {
                    if !visibility.get_point(coord).is_empty() {
                        any_revealed = true;
                        points_played.push(coord);
                    }
                    *visibility.point_mut(coord) = Bitmap::new();
                }
                if !point.is_empty() {
                    continue;
                }
                *point = active_seat.team;
                points_played.push(coord);
                any_placed = true;
            }
            if !any_placed {
                if any_revealed {
                    self.last_stone = Some(points_played);
                    return Ok(GroupVec::new());
                }
                return Err(MakeActionError::PointOccupied);
            }
        } else {
            if !shared.board.point_within((x, y)) {
                return Err(MakeActionError::OutOfBounds);
            }

            // TODO: don't repeat yourself
            let point = shared.board.point_mut((x, y));
            let revealed = if let Some(visibility) = &mut shared.board_visibility {
                let revealed = !visibility.get_point((x, y)).is_empty();
                *visibility.point_mut((x, y)) = Bitmap::new();
                revealed
            } else {
                false
            };
            if !point.is_empty() {
                if revealed {
                    self.last_stone = Some(tiny_vec![[Point; 8] => (x, y)]);
                    return Ok(points_played);
                }
                return Err(MakeActionError::PointOccupied);
            }

            *point = active_seat.team;
            points_played.push((x, y));
        }

        Ok(points_played)
    }

    fn capture(
        &self,
        shared: &mut SharedState,
        points_played: &mut GroupVec<Point>,
    ) -> (usize, Revealed) {
        let active_seat = get_active_seat(shared);
        let mut captures = 0;
        let mut revealed = false;

        let groups = find_groups(&shared.board);
        let dead_opponents = groups
            .iter()
            .filter(|g| g.liberties == 0 && g.team != active_seat.team);

        let board = &mut shared.board;

        for group in dead_opponents {
            for point in &group.points {
                *board.point_mut(*point) = Color::empty();
                captures += 1;
            }
            let reveals = reveal_group(&mut shared.board_visibility, group, board);
            revealed = revealed || reveals;

            if let Some(ponnuki) = shared.mods.ponnuki_is_points {
                if group.points.len() == 1
                    && board
                        .surrounding_points(group.points[0])
                        .all(|p| board.get_point(p) == active_seat.team)
                {
                    shared.points[active_seat.team.0 as usize - 1] += ponnuki;
                }
            }
        }

        // TODO: only re-scan own previously dead grouos
        let groups = find_groups(board);
        let dead_own = groups
            .iter()
            .filter(|g| g.liberties == 0 && g.team == active_seat.team);

        for group in dead_own {
            for point in &group.points {
                if points_played.contains(point) {
                    points_played.retain(|x| x != point);
                    *board.point_mut(*point) = Color::empty();
                }
            }
            let reveals = reveal_group(&mut shared.board_visibility, group, board);
            revealed = revealed || reveals;
        }

        (captures, revealed)
    }

    /// Superko
    /// We only need to scan back capture_count boards, as per Ten 1p's clever idea.
    /// The board can't possibly repeat further back than the number of removed stones.
    fn superko(
        &self,
        shared: &mut SharedState,
        captures: usize,
        hash: u64,
    ) -> MakeActionResult<()> {
        for BoardHistory {
            hash: old_hash,
            board: old_board,
            ..
        } in shared
            .board_history
            .iter()
            .rev()
            .take(shared.capture_count + captures)
        {
            if *old_hash == hash && old_board == &shared.board {
                let BoardHistory {
                    board: old_board,
                    points: old_points,
                    ..
                } = shared
                    .board_history
                    .last()
                    .expect("board_history.last() shouldn't be None")
                    .clone();
                shared.board = old_board;
                shared.points = old_points;
                return Err(MakeActionError::Ko);
            }
        }

        Ok(())
    }

    pub fn make_action_place(
        &mut self,
        shared: &mut SharedState,
        (x, y): (u32, u32),
    ) -> MakeActionResult {
        // TODO: should use some kind of set to make suicide prevention faster
        let mut points_played = self.place_stone(shared, (x, y))?;
        if points_played.is_empty() {
            return Ok(ActionChange::None);
        }

        let (captures, revealed) = self.capture(shared, &mut points_played);

        if points_played.is_empty() {
            let BoardHistory { board, points, .. } = shared
                .board_history
                .last()
                .expect("board_history.last() shouldn't be None")
                .clone();
            shared.board = board;
            shared.points = points;

            if revealed {
                return Ok(ActionChange::None);
            }
            return Err(MakeActionError::Suicide);
        }

        let hash = shared.board.hash();

        self.superko(shared, captures, hash)?;

        let new_turn = if let Some(rule) = &shared.mods.n_plus_one {
            use n_plus_one::NPlusOneResult::*;
            match n_plus_one::check(
                &points_played,
                &shared.board,
                shared.board_visibility.as_mut(),
                rule,
            ) {
                ExtraTurn => true,
                Nothing => false,
            }
        } else {
            false
        };

        if !new_turn {
            shared.turn += 1;
            if shared.turn >= shared.seats.len() {
                shared.turn = 0;
            }
        }

        self.last_stone = Some(points_played);
        for passed in &mut self.players_passed {
            *passed = false;
        }

        shared.board_history.push(BoardHistory {
            hash,
            board: shared.board.clone(),
            board_visibility: shared.board_visibility.clone(),
            state: GameState::Play(self.clone()),
            points: shared.points.clone(),
            turn: shared.turn,
        });
        shared.capture_count += captures;

        Ok(ActionChange::None)
    }

    pub fn make_action_pass(&mut self, shared: &mut SharedState) -> MakeActionResult {
        let active_seat = get_active_seat(shared);

        for (seat, passed) in shared.seats.iter().zip(self.players_passed.iter_mut()) {
            if seat.team == active_seat.team {
                *passed = true;
            }
        }

        shared.turn += 1;
        if shared.turn >= shared.seats.len() {
            shared.turn = 0;
        }

        shared.board_history.push(BoardHistory {
            hash: shared.board.hash(),
            board: shared.board.clone(),
            board_visibility: shared.board_visibility.clone(),
            state: GameState::Play(self.clone()),
            points: shared.points.clone(),
            turn: shared.turn,
        });

        if self.players_passed.iter().all(|x| *x) {
            for passed in &mut self.players_passed {
                *passed = false;
            }
            return Ok(ActionChange::PushState(GameState::scoring(
                &shared.board,
                shared.seats.len(),
                &shared.points,
            )));
        }

        Ok(ActionChange::None)
    }

    pub fn make_action_cancel(&mut self, shared: &mut SharedState) -> MakeActionResult {
        // Undo a turn
        if shared.board_history.len() < 2 {
            return Err(MakeActionError::OutOfBounds);
        }

        shared
            .board_history
            .pop()
            .ok_or(MakeActionError::OutOfBounds)?;
        let history = shared
            .board_history
            .last()
            .ok_or(MakeActionError::OutOfBounds)?;

        shared.board = history.board.clone();
        shared.board_visibility = history.board_visibility.clone();
        shared.points = history.points.clone();
        shared.turn = history.turn;

        *self = history.state.assume::<PlayState>().clone();

        Ok(ActionChange::None)
    }

    pub fn make_action(
        &mut self,
        shared: &mut SharedState,
        player_id: u64,
        action: ActionKind,
    ) -> MakeActionResult {
        let active_seat = get_active_seat(shared);
        if active_seat.player != Some(player_id) {
            return Err(MakeActionError::NotTurn);
        }

        let res = match action {
            ActionKind::Place(x, y) => self.make_action_place(shared, (x, y)),
            ActionKind::Pass => self.make_action_pass(shared),
            ActionKind::Cancel => self.make_action_cancel(shared),
        };

        let res = res?;

        self.set_zen_teams(shared);

        Ok(res)
    }

    fn set_zen_teams(&mut self, shared: &mut SharedState) {
        let move_number = shared.board_history.len() - 1;
        if let Some(zen) = &shared.mods.zen_go {
            for seat in &mut shared.seats {
                seat.team = Color((move_number % zen.color_count as usize) as u8 + 1);
            }
        }
    }
}

fn get_active_seat(shared: &SharedState) -> Seat {
    shared
        .seats
        .get(shared.turn)
        .expect("Game turn number invalid")
        .clone()
}

fn reveal_group(
    visibility: &mut Option<VisibilityBoard>,
    group: &Group,
    board: &Board,
) -> Revealed {
    let mut revealed = false;

    if let Some(visibility) = visibility {
        for &point in &group.points {
            revealed = revealed || !visibility.get_point(point).is_empty();
            *visibility.point_mut(point) = Bitmap::new();
            for point in board.surrounding_points(point) {
                revealed = revealed || !visibility.get_point(point).is_empty();
                *visibility.point_mut(point) = Bitmap::new();
            }
        }
    }

    revealed
}
