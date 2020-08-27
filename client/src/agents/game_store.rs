use yew::agent::{Agent, AgentLink};
use yew::prelude::*;
use yewtil::store::{Bridgeable, ReadOnly};
use yewtil::store::{Store, StoreWrapper};

use itertools::Itertools;

use crate::game_view::GameView;
use crate::networking;
use crate::utils;
use shared::game::GameHistory;
use shared::message::{ClientMessage, GameAction};

#[derive(Debug)]
pub enum Request {
    SetGame(GameView),
    SetGameHistory(Option<GameHistory>),
    GetBoardAt(u32),
    ScanBoard(i32),
}

#[derive(Debug)]
pub enum Action {
    SetGame(GameView),
    SetGameHistory(Option<GameHistory>),
    SetHistoryPending(u32, bool),
}

pub struct GameStore {
    pub game: Option<GameView>,
    pub history: Vec<Option<GameHistory>>,
    pub history_pending: bool,
    pub wanted_history: Option<u32>,
}

pub type GameStoreBridge = Box<dyn Bridge<StoreWrapper<GameStore>>>;

impl Store for GameStore {
    type Action = Action;
    type Input = Request;

    fn new() -> Self {
        GameStore {
            game: None,
            history: Vec::new(),
            history_pending: false,
            wanted_history: None,
        }
    }

    fn handle_input(&self, link: AgentLink<StoreWrapper<Self>>, msg: Self::Input) {
        match msg {
            Request::SetGame(game) => {
                utils::set_hash(&game.room_id.to_string());
                link.send_message(Action::SetGame(game));
            }
            Request::SetGameHistory(view) => {
                link.send_message(Action::SetGameHistory(view));
            }
            Request::GetBoardAt(turn) => {
                if self.history_pending {
                    link.send_message(Action::SetHistoryPending(turn, true));
                    return;
                }
                // TODO: This code needs a complete rework.
                let min = self
                    .history
                    .iter()
                    .find_position(|x| x.is_none())
                    .map(|x| x.0)
                    .unwrap_or(0);
                let max = self
                    .history
                    .iter()
                    .rev()
                    .find_position(|x| x.is_none())
                    .map(|x| self.history.len() - x.0)
                    .unwrap_or(0);
                if (max as u32 <= turn + 5 && max as u32 >= turn)
                    || self.history.len() <= turn as usize + 5
                {
                    networking::send(ClientMessage::GameAction(GameAction::BoardAt(
                        min as _,
                        if max > 0 {
                            (turn + 10).min(max as u32)
                        } else {
                            turn + 10
                        },
                    )));
                    link.send_message(Action::SetHistoryPending(turn, true));
                }
                if let Some(view) = self.history.get(turn as usize).cloned().flatten() {
                    link.send_message(Action::SetHistoryPending(turn, false));
                    link.send_message(Action::SetGameHistory(Some(view)));
                }
            }
            Request::ScanBoard(diff) => {
                let game = match &self.game {
                    Some(g) => g,
                    None => return,
                };
                let mut turn = match self.wanted_history {
                    None => game.move_number as i32 + diff,
                    Some(turn) => turn as i32 + diff,
                };
                if turn < 0 {
                    turn = 0;
                }
                if turn > game.move_number as i32 {
                    return;
                }

                self.handle_input(link, Request::GetBoardAt(turn as u32));
            }
        }
    }

    fn reduce(&mut self, msg: Self::Action) {
        match msg {
            Action::SetGame(game) => {
                let room_id = game.room_id;
                let move_number = game.move_number;
                let old = std::mem::replace(&mut self.game, Some(game));
                if let Some(old) = old {
                    if old.room_id == room_id {
                        self.game.as_mut().unwrap().history = old.history;
                        if move_number <= self.history.len() as u32 {
                            self.history.drain(move_number as usize..);
                        }
                    } else {
                        self.history.clear();
                        self.history_pending = false;
                        self.wanted_history = None;
                    }
                }
            }
            Action::SetGameHistory(view) => {
                if let Some(game) = &mut self.game {
                    if let Some(view) = &view {
                        while self.history.len() <= view.move_number as usize {
                            self.history.push(None);
                        }
                        self.history[view.move_number as usize] = Some(view.clone());
                        if Some(view.move_number) != self.wanted_history {
                            return;
                        }
                        self.history_pending = false;
                        if view.move_number == game.move_number {
                            game.history = None;
                            return;
                        }
                    } else {
                        self.history_pending = false;
                    }
                    game.history = view;
                }
            }
            Action::SetHistoryPending(turn, pending) => {
                if pending {
                    self.history_pending = true;
                }
                self.wanted_history = Some(turn);
            }
        }
    }
}

impl GameStore {
    pub fn bridge(cb: Callback<<StoreWrapper<Self> as Agent>::Output>) -> GameStoreBridge {
        <Self as Bridgeable>::bridge(cb)
    }

    pub fn set_game(bridge: &mut GameStoreBridge, game: GameView) {
        bridge.send(Request::SetGame(game));
    }

    pub fn set_game_history(bridge: &mut GameStoreBridge, view: Option<GameHistory>) {
        bridge.send(Request::SetGameHistory(view));
    }

    pub fn get_board_at(bridge: &mut GameStoreBridge, turn: u32) {
        bridge.send(Request::GetBoardAt(turn));
    }

    pub fn scan_board(bridge: &mut GameStoreBridge, amount: i32) {
        bridge.send(Request::ScanBoard(amount));
    }
}
