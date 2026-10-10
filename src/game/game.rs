use std::time::Duration;

use enum_map::EnumMap;
use rand::{Rng, seq::SliceRandom};
use serde::{Deserialize, Serialize};
use strum::{EnumCount, IntoEnumIterator};

use crate::{components::LocalStorage, game::{Board, BoardPos, Card, DECK_SIZE, DepotRole, NUM_RANKS, RANKS, SettingsState, Skin, Suit}};

pub const ANIMATION_DURATION: Duration = Duration::from_millis(200);
pub type AnimationKey = u16;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ActionRecord {
    pos1: BoardPos, pos2: BoardPos,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ScreenState {
    #[default] Game, 
    Settings, Help,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct GameState {
    pub board: Board,
    pub deal: Vec<Card>,
    #[serde(skip)]
    pub animation_key: AnimationKey, // used for syncing and to provide animator components with cycling keys
    pub history: Vec<ActionRecord>,
    pub undo_stack: Vec<usize>,
    pub already_won: bool,
    pub num_wins: i32,

    pub screen_state: ScreenState,

    pub allow_undo: bool,
    pub auto_play: bool,
    pub skin: Skin,
}

impl GameState {
    pub fn new_deal(rng: &mut impl Rng) -> Vec<Card> {
        let mut deck = Vec::with_capacity(DECK_SIZE);
        for rank in RANKS {
            for suit in Suit::iter() {
                deck.push(Card { rank, suit });
            }
        }

        deck.shuffle(rng);
        deck
    }

    pub fn init() -> Self {
        let mut res = Self {
            board: Board::empty(),
            deal: vec![],
            animation_key: 0,
            history: vec![],
            undo_stack: vec![],
            already_won: false,
            num_wins: 0,
            screen_state: ScreenState::Game,
            allow_undo: true,
            auto_play: true,
            skin: Skin::default(),
        };

        res.new_game();

        //// to test column limit rendering
        // for _ in 0..NUM_RANKS - 1 {
        //     res.board.depots[DepotRole::Tableau.id(1)].push(Card { rank: 12, suit: Suit::Spades })
        // }

        res
    }

    pub fn new_game(&mut self) {
        let deal = Self::new_deal(&mut rand::rng());
        self.board = Board::from_deal(&deal);
        self.deal = deal;
        self.history.clear();
        self.undo_stack.clear();
        self.already_won = false;
        LocalStorage.save_game_state(&self);
    }

    pub fn can_stack(&self, back: Card, front: Card) -> bool {
        back.suit.color() != front.suit.color() && front.rank + 1 == back.rank
    }

    pub fn can_sort(&self, back: Card, front: Card) -> bool {
        back.suit == front.suit && back.rank + 1 == front.rank
    }

    pub fn can_select(&self, pos: BoardPos) -> bool {
        let depot = pos.depot_index;
        let ord = pos.card_index;

        if ord >= self.board.depots[depot].len() {
            return false;
        }
        let slice = &self.board.depots[depot][ord..];

        let Some(role) = DepotRole::role(depot) else { return false };
        match role {
            DepotRole::Foundation => false,
            DepotRole::FreeCell => { slice.len() <= 1 },
            DepotRole::Tableau => {
                slice.windows(2).all(|w| self.can_stack(w[0], w[1]))
            },
        }
    }

    pub fn can_move(&self, pos1: BoardPos, pos2: BoardPos) -> bool {
        if pos1.depot_index == pos2.depot_index { return false; }
        let depot1 = &self.board.depots[pos1.depot_index];
        let depot2 = &self.board.depots[pos2.depot_index];
        let num_moved = depot1.len() - pos1.card_index;
        if pos2.card_index != depot2.len() { return false; }

        let card = depot1[pos1.card_index];
        let Some(role) = DepotRole::role(pos2.depot_index) else { return false };
        match role {
            DepotRole::Foundation => {
                num_moved == 1 && if let Some(&c) = depot2.last() {
                    self.can_sort(c, card)
                } else {
                    1 == card.rank
                }
            },
            DepotRole::FreeCell => {
                depot2.is_empty() && num_moved == 1 
                    && DepotRole::wing(pos1.depot_index) == DepotRole::wing(pos2.depot_index)
            },
            DepotRole::Tableau => {
                if let Some(&c) = depot2.last() {
                    DepotRole::wing(pos1.depot_index) != DepotRole::wing(pos2.depot_index) &&
                        self.can_stack(c, card)
                } else {
                    true
                }
            },
        }
    }

    pub fn undo_possible(&self) -> bool {
        self.allow_undo && !self.history.is_empty()
    }

    pub fn undo(&mut self) {
        if self.is_busy() || !self.undo_possible() { return; }
        let Some(target_len) = self.undo_stack.pop() else {return};
        while self.history.len() > target_len {
            let rec = self.history.pop().unwrap();
            self.board.do_move(rec.pos2, rec.pos1);
            self.board.advance_actions(); // no animation, as repeated card moves on same card causes problems
        }
        LocalStorage.save_game_state(&self);
    }

    pub fn restart(&mut self) {
        if self.history.is_empty() || !self.undo_possible() { return; }
        self.board = Board::from_deal(&self.deal);
        self.history.clear();
        self.undo_stack.clear();
        if !self.is_busy() { LocalStorage.save_game_state(&self); }
    }

    pub fn is_busy(&self) -> bool {
        self.is_acting()
    }

    pub fn is_acting(&self) -> bool {
        !self.board.animation_acts.is_empty()
    }

    pub fn is_won(&self) -> bool {
        use DepotRole::*;
        Foundation.range().all(|d| self.board.depots[d].len() == NUM_RANKS)
    }

    /// returns an EnumMap where each suit gives the rank that is safe to sort
    pub fn get_safe_sorts(&self) -> EnumMap<Suit, u8> {
        // first get the ranks of the cards that are already sorted
        let mut foundation_ranks = EnumMap::<Suit, u8>::default();
        for i in DepotRole::Foundation.range() {
            if let Some(card) = self.board.depots[i].last() {
                foundation_ranks[card.suit] = card.rank;
            }
        }

        // then going from the suit with the lowest ranks sorted to highest, check if the cards that may be placed on
        // the candidate are either already sorted, or would be safe to sort once uncovered
        let mut ite = Suit::iter();
        let mut order: [Suit; Suit::COUNT] = std::array::from_fn(|_| ite.next().unwrap());
        order.sort_by_key(|&s| foundation_ranks[s]);
        for s in order {
            let ans = foundation_ranks.iter().all(|(other, &rank)| {
                other.color() == s.color() || rank >= foundation_ranks[s]
            });
            if ans { foundation_ranks[s] += 1; }
        }
        foundation_ranks
    }

    fn do_move_raw(&mut self, pos1: BoardPos, pos2: BoardPos) {
        self.board.do_move(pos1, pos2);
        self.history.push(ActionRecord { pos1, pos2 })
    }

    pub fn check_auto_moves(&mut self) {
        if self.is_busy() { return; }
        if !self.auto_play { return; }

        let safe_sorts = self.get_safe_sorts();
        let it = [
            DepotRole::FreeCell,
            DepotRole::Tableau,
        ].iter().flat_map(|r| r.range());

        for depot in it {
            if let Some(card) = self.board.depots[depot].last() {
                if safe_sorts[card.suit] != card.rank { continue; }
                let src = BoardPos::new(depot, self.board.depots[depot].len() - 1);
                for dest in DepotRole::Foundation.range() {
                    let dest = BoardPos::new(dest, self.board.depots[dest].len());
                    if !self.can_move(src, dest) { continue; }
                    self.do_move_raw(src, dest);
                    return;
                }
            }
        }
    }

    pub fn onclick(&mut self, pos: BoardPos) {
        if self.is_busy() { return; }

        if let Some(src) = self.board.selected {
            if pos == src { 
                self.board.selected = None; 
                return;
            }
            if src.depot_index == pos.depot_index && self.can_select(pos) {
                self.board.selected = Some(pos);
                return;
            }

            let dest = BoardPos::new(pos.depot_index, pos.card_index.wrapping_add(1));
            if !self.can_move(src, dest) { return; }
            self.undo_stack.push(self.history.len());
            self.do_move_raw(src, dest);
        } else {
            if self.can_select(pos) {
                self.board.selected = Some(pos);
            }
        }
    }

    pub fn ondoubleclick(&mut self, pos: BoardPos) {
        if self.is_busy() { return; }
        if !self.can_select(pos) { return; } // needed, or illegal stacks can still be moved this way!
        let it = DepotRole::Foundation.range().chain(DepotRole::FreeCell.range());
        for dest in it {
            let dest = BoardPos::new(dest, self.board.depots[dest].len());
            if self.can_move(pos, dest) {
                self.undo_stack.push(self.history.len());
                self.do_move_raw(pos, dest);
                return;
            }
        }
    }

    pub fn advance_animations(&mut self, key: AnimationKey) {
        if key != self.animation_key { return; }
        self.animation_key = self.animation_key.wrapping_add(1);
        
        self.board.advance_actions();

        if self.is_won() {
            if !self.already_won {
                self.num_wins += 1;
                self.already_won = true;
            }
        } else {
            self.check_auto_moves();
        }

        if !self.is_busy() { LocalStorage.save_game_state(&self); }
    }

    pub fn new_settings_state(&self) -> SettingsState {
        SettingsState {
            allow_undo: self.allow_undo,
            auto_play: self.auto_play,
            skin: self.skin,
        }
    }

    pub fn apply_settings(&mut self, settings: &SettingsState){
        self.allow_undo = settings.allow_undo;
        self.auto_play = settings.auto_play;
        self.skin = settings.skin;
        LocalStorage.save_game_state(&self);
    }
}