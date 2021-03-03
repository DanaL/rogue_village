// This file is part of RogueVillage, a roguelike game.
//
// RogueVillage is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// RogueVillage is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with RogueVillage.  If not, see <https://www.gnu.org/licenses/>.

extern crate serde;

use std::{collections::{HashMap, HashSet, VecDeque}, u128};
use std::time::Instant;

use rand::thread_rng;
use rand::Rng;
use serde::{Serialize, Deserialize};
//use std::time::{Duration, Instant};

use super::{EventResponse, EventType, GameObjects, GameState};

use crate::{dialogue, land_on_location};
use crate::dialogue::DialogueLibrary;
use crate::display;
use crate::game_obj::{GameObject, GameObjType};
use crate::items::{GoldPile, Item};
use crate::map::{Tile, DoorState, SpecialSquare};
use crate::pathfinding::find_path;
use crate::player::Player;
use crate::util;
use crate::util::StringUtils;
use crate::fov;

// Some bitmasks for various monster attributes
pub const MA_OPEN_DOORS: u128       = 0b00000001;
pub const MA_UNLOCK_DOORS: u128     = 0b00000010;
pub const MA_WEAK_VENOMS: u128      = 0b00000100;
pub const MA_PACK_TACTICS: u128     = 0b00001000;
pub const MA_FEARLESS: u128         = 0b00010000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Venue {
    TownSquare,
    Tavern,
    Shrine,
    Favourite((i32, i32, i8)),
    Visit(i32),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgendaItem {
    pub from: (u16, u16),
    pub to: (u16, u16),
    pub priority: u8,
    pub place: Venue,
}

impl AgendaItem {
    pub fn new(from: (u16, u16), to: (u16, u16), priority: u8, place: Venue) -> AgendaItem {
        AgendaItem { from, to, priority, place, }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Copy, Serialize, Deserialize)]
pub enum Attitude {
    Stranger,
    Indifferent,
    Friendly,
    Hostile,
    Fleeing,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Move((i32, i32, i8)),
    OpenDoor((i32, i32, i8)),
    CloseDoor((i32, i32, i8)),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum NPCMode {
    Villager,
    SimpleMonster,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NPC {
    pub name: String,
    pub ac: u8,
	pub max_hp: u8,
	pub curr_hp: u8,
	pub location: (i32, i32, i8),
    pub ch: char,
    pub color: (u8, u8, u8),
    pub attitude: Attitude,
    pub facts_known: Vec<usize>,
    pub home_id: usize,
    pub plan: VecDeque<Action>,
    pub voice: String,
    pub schedule: Vec<AgendaItem>,
    pub object_id: usize,
    pub mode: NPCMode,
    pub attack_mod: u8,
    pub dmg_dice: u8,
    pub dmg_die: u8,
    pub dmg_bonus: u8,
    pub edc: u8,
    pub attributes: u128,
}

impl NPC {
    pub fn villager(name: String, location: (i32, i32, i8), home_id: usize, voice: &str, object_id: usize) -> NPC {
        NPC { name, ac: 10, curr_hp: 8, max_hp: 8, location, ch: '@', color: display::LIGHT_GREY, attitude: Attitude::Stranger, 
            facts_known: Vec::new(), home_id, plan: VecDeque::new(), voice: String::from(voice), 
            schedule: Vec::new(), object_id, mode: NPCMode::Villager, attack_mod: 2, dmg_dice: 1, dmg_die: 3, dmg_bonus: 0, edc: 12,
            attributes: MA_OPEN_DOORS | MA_UNLOCK_DOORS,
        }
    }
    
    // I should be able to move calc_plan_to_move, try_to_move_to_loc, etc to generic
    // places for all Villager types since they'll be pretty same-y. The differences
    // will be in how NPCs set their plans/schedules. 
    fn calc_plan_to_move(&mut self, state: &GameState, goal: (i32, i32, i8), stop_before: bool) {
        if self.plan.len() == 0 {
            let mut passable = HashMap::new();
            passable.insert(Tile::Grass, 1.0);
            passable.insert(Tile::Dirt, 1.0);
            passable.insert(Tile::Tree, 1.0);
            passable.insert(Tile::Door(DoorState::Open), 1.0);
            passable.insert(Tile::Door(DoorState::Broken), 1.0);
            if self.attributes & MA_OPEN_DOORS > 0 {
                passable.insert(Tile::Door(DoorState::Closed), 2.0);
            }
            if self.attributes & MA_UNLOCK_DOORS > 0 {
                passable.insert(Tile::Door(DoorState::Locked), 2.5);
            }
            passable.insert(Tile::StoneFloor, 1.0);
            passable.insert(Tile::Floor, 1.0);
            passable.insert(Tile::Trigger, 1.0);

            let mut path = find_path(&state.map, stop_before, self.location.0, self.location.1, 
                self.location.2, goal.0, goal.1, 50, &passable);
            
            path.pop(); // first square in path is the start location
            while path.len() > 0 {
                let sq = path.pop().unwrap();
                self.plan.push_back(Action::Move((sq.0, sq.1, self.location.2)));
            }
        }
    }

    fn try_to_move_to_loc(&mut self, loc: (i32, i32, i8), state: &mut GameState, game_objs: &mut GameObjects) {        
        if game_objs.blocking_obj_at(&loc) || state.player_loc == loc {
            state.write_msg_buff("\"Excuse me.\"");
            self.plan.push_front(Action::Move(loc));
        } else if state.map[&loc] == Tile::Door(DoorState::Closed) {
            self.plan.push_front(Action::Move(loc));
            self.open_door(loc, state);
        } else {
            // Villagers will close doors after they pass through them, although monsters in the dungeon 
            // shouldn't for the most part.
            if !(self.attitude == Attitude::Hostile || self.attitude == Attitude::Fleeing) {
                if let Tile::Door(DoorState::Open) = state.map[&self.get_location()] {
                    self.plan.push_front(Action::CloseDoor(self.get_location()));                
                }
            }
            self.location = loc;
            land_on_location(state, game_objs, loc, self.get_object_id());
        }
    }

    fn open_door(&mut self, loc: (i32, i32, i8), state: &mut GameState) {
        let s = format!("{} opens the door.", self.get_fullname().with_def_article().capitalize());
        state.write_msg_buff(&s);
        state.map.insert(loc, Tile::Door(DoorState::Open));
    }

    fn close_door(&mut self, loc: (i32, i32, i8), state: &mut GameState, game_objs: &mut GameObjects) {
        if game_objs.blocking_obj_at(&loc) || loc == state.player_loc {
            state.write_msg_buff("Please don't stand in the doorway.");
            self.plan.push_front(Action::CloseDoor(loc));
        } else {
            if let Tile::Door(DoorState::Open) = state.map[&loc] {
                if self.attitude == Attitude::Stranger {
                    state.write_msg_buff("The villager closes the door.");
                } else {
                    let s = format!("{} closes the door.", self.get_fullname());
                    state.write_msg_buff(&s);
                }
                state.map.insert(loc, Tile::Door(DoorState::Closed));
            }
        }
    }

    fn follow_plan(&mut self, state: &mut GameState, game_objs: &mut GameObjects) {
        let action = self.plan.pop_front().unwrap();
        match action {
            Action::Move(loc) => self.try_to_move_to_loc(loc, state, game_objs),
            Action::OpenDoor(loc) => self.open_door(loc, state),
            Action::CloseDoor(loc) => self.close_door(loc, state, game_objs),
        }
    }

    // fn is_home_open(&self, map: &Map) -> bool {
    //     match self.entrance_location(map) {
    //         Some(loc) => 
    //             if map[&loc] == Tile::Door(DoorState::Open) {
    //                 true
    //             } else {
    //                 false
    //             },
    //         _ => false
    //     }        
    // }

    fn idle_behaviour(&mut self, state: &GameState) {
        // If the NPC doesn't need to move anywhere, just pick an adjacent square to step to sometimes.
        // (Maybe eventually if they are adjacent to another NPC, have them make small talk?)
        if thread_rng().gen_range(0.0, 1.0) < 0.33 {
            let j = thread_rng().gen_range(0, util::ADJ.len()) as usize;
            let d = util::ADJ[j];
            let adj = (self.location.0 + d.0, self.location.1 + d.1, self.location.2);
            if state.map[&adj].passable_dry_land() {
                self.calc_plan_to_move(state, adj, false);
            }
        }
    }

    fn check_agenda_item(&mut self, state: &GameState, item: &AgendaItem) {        
        match item.place {
            Venue::Tavern => {
                let tavern = &state.world_info.town_buildings.as_ref().unwrap().tavern;
                if !in_location(state, self.get_location(), &tavern, true) {
                    self.go_to_place(state, tavern);
                } else {
                    self.idle_behaviour(state);
                }
            },
            Venue::TownSquare => {
                let ts = &state.world_info.town_square;
                if !in_location(state, self.get_location(), ts, false) {
                    self.go_to_place(state, ts);
                } else {
                    self.idle_behaviour(state);
                }
            },
            _ => {
                // Eventually I'll implement the other venues...
            },
        }
    }

    fn villager_schedule(&mut self, state: &GameState) {
        let ct = state.curr_time();
        let minutes = ct.0 * 60 + ct.1;
        
        // Select the current, highest priority agenda item from the schedule
        let mut items: Vec<&AgendaItem> = self.schedule.iter()
                     .filter(|i| i.from.0 * 60 + i.from.1 <= minutes && minutes <= i.to.0 * 60 + i.to.1)
                     .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        if items.len() == 0 {
            // The default behaviour is to go home if nothing on the agenda.
            let b = &state.world_info.town_buildings.as_ref().unwrap();
            if !in_location(state, self.get_location(), &b.homes[self.home_id], true) {
                self.go_to_place(state, &b.homes[self.home_id]);
            } else {
                self.idle_behaviour(state);
            }            
        } else {
            let item = &items[0].clone();
            self.check_agenda_item(state, item);
        }
    }

    fn simple_monster_schedule(&mut self, state: &GameState) {
        if self.attitude != Attitude::Hostile {
            // Can I see the player? if so, become hostile
            let m_fov_time = Instant::now();
            let sqs = fov::calc_fov(state, self.location, 10, true);
            let m_fov_elapsed = m_fov_time.elapsed();
            println!("Monster fov: {:?}", m_fov_elapsed);

            let visible: HashSet<(i32, i32, i8)> = sqs.iter().filter(|sq| sq.1).map(|sq| sq.0).collect();
            if visible.contains(&state.player_loc) {
                self.attitude = Attitude::Hostile;
            }
        }

        if self.attitude == Attitude::Hostile {
            let m_pf_time = Instant::now();
            self.calc_plan_to_move(state, state.player_loc, true);
            let m_pf_elapsed = m_pf_time.elapsed();
            println!("Monster pf time: {:?}", m_pf_elapsed);
        }        
    }

    fn check_schedule(&mut self, state: &GameState) {
        // I feel like there HAS to be way a better way to do polymorphism/different behaviours in Rust. I
        // feel like Traits will be too much of a pain with the GameObjs and I couldn't really share code between the 
        // NPC types. Unless I make them floating functions and have no private fields?
        match self.mode {
            NPCMode::Villager => self.villager_schedule(state),
            NPCMode::SimpleMonster => self.simple_monster_schedule(state),
        }
    }

    // Generally, when I have an NPC go a building/place, I assume it doesn't matter too much if 
    // they go to specific square inside it, so just pick any one of them.
    fn go_to_place(&mut self, state: &GameState, sqs: &HashSet<(i32, i32, i8)>) {
        let j = thread_rng().gen_range(0, &sqs.len());
        let goal_loc = &sqs.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
        self.calc_plan_to_move(state, *goal_loc, false);
    }
}

fn in_location(state: &GameState, loc: (i32, i32, i8), sqs: &HashSet<(i32, i32, i8)>, indoors: bool) -> bool {
    if indoors {
        let indoor_sqs = HashSet::from(sqs.iter()
                                          .filter(|sq| state.map[&sq].indoors())
                                          .collect::<HashSet<&(i32, i32, i8)>>());
        indoor_sqs.contains(&loc)
    } else {
        sqs.contains(&loc)
    }
}

pub fn pick_villager_name(used_names: &HashSet<String>) -> String {
    let names: [&str; 12] = ["Galleren", "Jaquette", "Aalis", "Martin", "Brida", "Cecillia",
        "Gotleib", "Ulrich", "Magda", "Sofiya", "Milivoj", "Velimer"];

    loop {
        let n = thread_rng().gen_range(0, names.len());
        if !used_names.contains(names[n]) {
            return String::from(names[n]);
        }
    }
}

impl GameObject for NPC {
    fn blocks(&self) -> bool {
        true
    }

    fn is_npc(&self) -> bool {
        true
    }

    fn get_location(&self) -> (i32, i32, i8) {
        self.location
    }

    fn set_location(&mut self, loc: (i32, i32, i8)) {
        self.location = loc;
    }

    fn receive_event(&mut self, _event: EventType, _state: &mut GameState) -> Option<EventResponse> {
        None
    }

    fn get_fullname(&self) -> String {
        self.name.clone()
    }

    fn get_object_id(&self) -> usize {
        self.object_id
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.color, self.ch)
    }

    fn get_type(&self) -> GameObjType {
        GameObjType::NPC
    }
    
    fn as_item(&self) -> Option<Item> {
        None
    }

    fn as_zorkmids(&self) -> Option<GoldPile> {
        None
    }
    
    fn as_villager(&self) -> Option<NPC> {
        Some(self.clone())
    }
    
    fn as_special_sq(&self) -> Option<SpecialSquare> {
        None
    }

    fn take_turn(&mut self, state: &mut GameState, game_objs: &mut GameObjects) {
        if self.plan.len() > 0 {
            self.follow_plan(state, game_objs);            
        } else {
            self.check_schedule(state);
        }        
    }

    fn talk_to(&mut self, state: &mut GameState, player: &Player, dialogue: &DialogueLibrary) -> String {
        if self.voice == "monster" {
            let s = format!("{} growls.", self.name.with_def_article().capitalize());
            return s;
        }

        let line = dialogue::parse_voice_line(&dialogue::pick_voice_line(dialogue, &self.voice, self.attitude), &state.world_info, player,
            &self.name, self.location);
        if self.attitude == Attitude::Stranger {
            // Perhaps a charisma check to possibly jump straight to friendly?
            self.attitude = Attitude::Indifferent;
        }

        line
    }

    fn hidden(&self) -> bool {
        false
    }

    fn reveal(&mut self) { }
    fn hide(&mut self) { }
}

// This could be in a data file and maybe one day will be but for now the compiler will help me avoid stupid typos
// in basic monster definitions!
pub struct MonsterFactory {
    // AC, HP, ch, colour, mode, attack_mod, dmg_dice, dmg_die, dmg_bonus, level, attributes
    table: HashMap<String, (u8, u8, char, (u8, u8, u8), NPCMode, u8, u8, u8, u8, u8, u128)>, 
}

impl MonsterFactory {
    pub fn init() -> MonsterFactory {
        let mut mf = MonsterFactory { table: HashMap::new() };

        mf.table.insert(String::from("kobold"), (13, 7, 'k', display::DULL_RED, NPCMode::SimpleMonster, 4, 1, 4, 2, 1,
            MA_OPEN_DOORS | MA_UNLOCK_DOORS | MA_PACK_TACTICS));
        mf.table.insert(String::from("goblin"), (15, 7, 'o', display::GREEN, NPCMode::SimpleMonster, 4, 1, 6, 2, 1,
            MA_OPEN_DOORS | MA_UNLOCK_DOORS));
        mf.table.insert(String::from("zombie"), (11, 8, 'z', display::GREY, NPCMode::SimpleMonster, 4, 1, 6, 2, 1,
            MA_OPEN_DOORS | MA_UNLOCK_DOORS | MA_FEARLESS));
        mf
    }

    fn calc_dc(&self, level: u8) -> u8 {
        if level < 5 {
            12
        } else if level < 8 {
            13
        } else if level <  11 {
            14
        } else if level < 14 {
            15
        } else if level < 17 {
            16
        } else {
            18
        }
    }

    pub fn add_monster(&self, name: &str, loc: (i32, i32, i8), game_objs: &mut GameObjects) {
        if !self.table.contains_key(name) {
            let s = format!("Unknown monster: {}!!", name);
            panic!(s);
        }

        let stats = self.table.get(name).unwrap();
        let obj_id = game_objs.next_id();
        let npc = NPC { name: String::from(name), ac: stats.0, curr_hp: stats.1, max_hp: stats.1, location: loc, ch: stats.2, 
            color: stats.3, attitude: Attitude::Indifferent, facts_known: Vec::new(), home_id: 0, plan: VecDeque::new(), 
            voice: String::from("monster"), schedule: Vec::new(), object_id: obj_id, mode: stats.4, attack_mod: stats.5, 
            dmg_dice: stats.6, dmg_die: stats.7, dmg_bonus: stats.8, edc: self.calc_dc(stats.9), attributes: stats.10,
        };

        game_objs.add(Box::new(npc));
        game_objs.listeners.insert((obj_id, EventType::TakeTurn));
    }
}
