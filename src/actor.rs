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

use super::{EventType, GameObjects, GameState};

use crate::{battle, player};
use crate::dialogue;
use crate::dialogue::DialogueLibrary;
use crate::display;
use crate::game_obj::GameObject;
use crate::items::GoldPile;
use crate::map::{Tile, DoorState};
use crate::pathfinding::find_path;
use crate::util;
use crate::util::StringUtils;
use crate::fov;

// Some bitmasks for various monster attributes
pub const MA_OPEN_DOORS: u128       = 0x00000001;
pub const MA_UNLOCK_DOORS: u128     = 0x00000002;
pub const MA_WEAK_VENOMS: u128      = 0x00000004;
pub const MA_PACK_TACTICS: u128     = 0x00000008;
pub const MA_FEARLESS: u128         = 0x00000010;
pub const MA_UNDEAD: u128           = 0x00000020;
pub const MA_RESIST_SLASH: u128     = 0x00000040;
pub const MA_RESIST_PIERCE: u128    = 0x00000080;

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
    Attack((i32, i32, i8)),
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum NPCPersonality {
    Villager,
    SimpleMonster,
    BasicUndead,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Behaviour {
    Idle,
    Hunt,
    Wander,
    Guard((i32, i32, i8)),
    Defend(usize),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NPC {
    pub name: String,
    pub ac: u8,
	pub max_hp: u8,
	pub curr_hp: u8,
	pub attitude: Attitude,
    pub facts_known: Vec<usize>,
    pub home_id: usize,
    pub plan: VecDeque<Action>,
    pub voice: String,
    pub schedule: Vec<AgendaItem>,
    pub mode: NPCPersonality,
    pub attack_mod: u8,
    pub dmg_dice: u8,
    pub dmg_die: u8,
    pub dmg_bonus: u8,
    pub edc: u8,
    pub attributes: u128,
    pub curr_loc: (i32, i32, i8),
    pub alive: bool, // as in function, HPs > 0, not indication of undead status
    pub xp_value: u32,
    pub inventory: Vec<GameObject>,
    pub active: bool,
    pub active_behaviour: Behaviour,
    pub inactive_behaviour: Behaviour,
}

impl NPC {
    pub fn villager(name: String, location: (i32, i32, i8), home_id: usize, voice: &str, game_objs: &mut GameObjects) -> GameObject {      
        let npc_name = name.clone();  
        let npc = NPC { name, ac: 10, curr_hp: 8, max_hp: 8, attitude: Attitude::Stranger, facts_known: Vec::new(), home_id, plan: VecDeque::new(), 
            voice: String::from(voice), schedule: Vec::new(), mode: NPCPersonality::Villager, attack_mod: 2, dmg_dice: 1, dmg_die: 3, dmg_bonus: 0, edc: 12,
            attributes: MA_OPEN_DOORS | MA_UNLOCK_DOORS, curr_loc: (-1, -1, -1), alive: true, xp_value: 0, inventory: Vec::new(),
            active: true, active_behaviour: Behaviour::Idle, inactive_behaviour: Behaviour::Idle,
        };

        let obj = GameObject::new(game_objs.next_id(), &npc_name, location, '@', display::LIGHT_GREY, display::LIGHT_GREY, 
            Some(npc), None , None, None, None, true);
		obj
    }
    
    // I should be able to move calc_plan_to_move, try_to_move_to_loc, etc to generic
    // places for all Villager types since they'll be pretty same-y. The differences
    // will be in how NPCs set their plans/schedules. 
    fn calc_plan_to_move(&mut self, state: &GameState, goal: (i32, i32, i8), stop_before: bool, my_loc: (i32, i32, i8)) {
        if self.plan.len() == 0 {
            let mut passable = HashMap::new();
            passable.insert(Tile::Grass, 1.0);
            passable.insert(Tile::Dirt, 1.0);
            passable.insert(Tile::Tree, 1.0);
            passable.insert(Tile::Door(DoorState::Open), 1.0);
            passable.insert(Tile::Door(DoorState::Broken), 1.0);
            passable.insert(Tile::Gate(DoorState::Open), 1.0);
            passable.insert(Tile::Gate(DoorState::Broken), 1.0);
            if self.attributes & MA_OPEN_DOORS > 0 {
                passable.insert(Tile::Door(DoorState::Closed), 2.0);
            }
            if self.attributes & MA_UNLOCK_DOORS > 0 {
                passable.insert(Tile::Door(DoorState::Locked), 2.5);
            }
            passable.insert(Tile::StoneFloor, 1.0);
            passable.insert(Tile::Floor, 1.0);
            passable.insert(Tile::Trigger, 1.0);
            passable.insert(Tile::Rubble, 1.50);

            let mut path = find_path(&state.map, stop_before, my_loc.0, my_loc.1, 
                my_loc.2, goal.0, goal.1, 50, &passable);
            
            path.pop(); // first square in path is the start location
            while path.len() > 0 {
                let sq = path.pop().unwrap();
                self.plan.push_back(Action::Move((sq.0, sq.1, my_loc.2)));
            }
        }
    }

    fn try_to_move_to_loc(&mut self, goal_loc: (i32, i32, i8), state: &mut GameState, game_objs: &mut GameObjects, my_loc: (i32, i32, i8)) {
        if goal_loc == my_loc {
            println!("Hmm I'm trying to move to my own location...");
        }   
        if game_objs.blocking_obj_at(&goal_loc) {
            match self.mode {
                NPCPersonality::Villager => {
                    state.write_msg_buff("\"Excuse me.\"");
                    self.plan.push_front(Action::Move(goal_loc));
                }
                _ => {
                    // if someone/something is blocking path, clear the current plan which should trigger creating a new plan
                    self.plan.clear();
                }
            }
            
        } else if state.map[&goal_loc] == Tile::Door(DoorState::Closed) {
            self.plan.push_front(Action::Move(goal_loc));
            self.open_door(goal_loc, state);
        } else {
            // Villagers will close doors after they pass through them
            if self.mode == NPCPersonality::Villager {
                if let Tile::Door(DoorState::Open) = state.map[&my_loc] {
                    self.plan.push_front(Action::CloseDoor(my_loc));                
                }
            }

            // Need to fix this up so that I'm not duplicating code from do_move() in main.rs as much as possible
            let curr_tile = state.map[&my_loc];
            if curr_tile == Tile::Rubble {
                let mut rng = rand::thread_rng();
                if rng.gen_range(1, 21) < 9 {
                    let s = format!("{} stumbles in the rubble.", self.npc_name(false).capitalize());
                    state.write_msg_buff(&s);
                    self.plan.push_front(Action::Move(goal_loc));
                    return;
                }                
            }

            self.curr_loc = goal_loc;            
        }
    }

    fn open_door(&mut self, loc: (i32, i32, i8), state: &mut GameState) {
        let s = format!("{} opens the door.", self.name.with_def_article().capitalize());
        state.write_msg_buff(&s);
        state.map.insert(loc, Tile::Door(DoorState::Open));
    }

    fn close_door(&mut self, loc: (i32, i32, i8), state: &mut GameState, game_objs: &mut GameObjects) {
        if game_objs.blocking_obj_at(&loc) {
            state.write_msg_buff("Please don't stand in the doorway.");
            self.plan.push_front(Action::CloseDoor(loc));
        } else {
            if let Tile::Door(DoorState::Open) = state.map[&loc] {
                if self.attitude == Attitude::Stranger {
                    state.write_msg_buff("The villager closes the door.");
                } else {
                    let s = format!("{} closes the door.", self.name);
                    state.write_msg_buff(&s);
                }
                state.map.insert(loc, Tile::Door(DoorState::Closed));
            }
        }
    }

    fn follow_plan(&mut self, my_id: usize, state: &mut GameState, game_objs: &mut GameObjects, my_loc: (i32, i32, i8)) {
        if let Some(action) = self.plan.pop_front() {
            match action {
                Action::Move(loc) => self.try_to_move_to_loc(loc, state, game_objs, my_loc),
                Action::OpenDoor(loc) => self.open_door(loc, state),
                Action::CloseDoor(loc) => self.close_door(loc, state, game_objs),
                Action::Attack(_loc) => {
                    battle::monster_attacks_player(state, self, my_id, game_objs);
                },
            }
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

    fn check_agenda_item(&mut self, state: &GameState, item: &AgendaItem, loc: (i32, i32, i8)) {        
        // match item.place {
        //     Venue::Tavern => {
        //         let tavern = &state.world_info.town_buildings.as_ref().unwrap().tavern;
        //         if !in_location(state, loc, &tavern, true) {
        //             self.go_to_place(state, tavern, loc);
        //         } else {
        //             self.idle_behaviour(state, loc);
        //         }
        //     },
        //     Venue::TownSquare => {
        //         let ts = &state.world_info.town_square;
        //         if !in_location(state, loc, ts, false) {
        //             self.go_to_place(state, ts, loc);
        //         } else {
        //             self.idle_behaviour(state, loc);
        //         }
        //     },
        //     _ => {
        //         // Eventually I'll implement the other venues...
        //     },
        // }
    }

    fn villager_schedule(&mut self, state: &GameState, loc: (i32, i32, i8)) {
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
            if !in_location(state, loc, &b.homes[self.home_id], true) {
                self.go_to_place(state, &b.homes[self.home_id], loc);
            } else {
                //self.idle_behaviour(state, loc);
            }
        } else {
            let item = &items[0].clone();
            self.check_agenda_item(state, item, loc);
        }
    }

    fn check_schedule(&mut self, state: &GameState, loc: (i32, i32, i8), player_loc: (i32, i32, i8)) {
        // I feel like there HAS to be way a better way to do polymorphism/different behaviours in Rust. I
        // feel like Traits will be too much of a pain with the GameObjs and I couldn't really share code between the 
        // NPC types. Unless I make them floating functions and have no private fields?
        
        // if let self.mode = NPCPersonality::Villager {
        //     self.villager_schedule(state, loc);
        // } 
    }

    // Generally, when I have an NPC go a building/place, I assume it doesn't matter too much if 
    // they go to specific square inside it, so just pick any one of them.
    fn go_to_place(&mut self, state: &GameState, sqs: &HashSet<(i32, i32, i8)>, my_loc: (i32, i32, i8)) {
        let j = thread_rng().gen_range(0, &sqs.len());
        let goal_loc = &sqs.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
        self.calc_plan_to_move(state, *goal_loc, false, my_loc);
    }

    fn random_adj_sq(&mut self, game_objs: &GameObjects, state: &GameState, loc: (i32, i32, i8)) {
        if thread_rng().gen_range(0.0, 1.0) < 0.33 {
            let j = thread_rng().gen_range(0, util::ADJ.len()) as usize;
            let d = util::ADJ[j];
            let adj = (loc.0 + d.0, loc.1 + d.1, loc.2);
            if !game_objs.blocking_obj_at(&adj) && state.map[&adj].passable_dry_land() {
                self.calc_plan_to_move(state, adj, false, loc);
            }
        }
    }

    // Quick, dirty guess of which adjacent, open square is closest to the player
    fn best_guess_toward_player(&mut self, state: &GameState, loc: (i32, i32, i8), player_loc: (i32, i32, i8)) -> (i32, i32, i8) {
        let mut nearest = i32::MAX;
        let mut best = loc;
        for adj in util::ADJ.iter() {            
            let a = (loc.0 + adj.0, loc.1 + adj.1, loc.2);

            // This will need to be updated when I add aquatic creatures
            if !state.map[&a].passable_dry_land() {
                continue;
            }

            let d = (a.0 - player_loc.0) * (a.0 - player_loc.0) + (a.1 - player_loc.1) * (a.1 - player_loc.1);
            if d < nearest {
                best = a;
                nearest = d;
            }
        }

        best
    }

    fn can_see_player(&mut self, state: &GameState, loc: (i32, i32, i8), player_loc: (i32, i32, i8)) -> bool {
        let dr = loc.0 - player_loc.0;
        let dc = loc.1 - player_loc.1;
        let d = dr * dr + dc * dc;

        // This distance check may be premature optimization. If monster fov turns out to not be a bottleneck
        // I can ditch it. But my first ever attempt at a roguelike was in Python in 2002 and you had to be
        // careful about speed...
        if d < 169 {
            let visible = fov::calc_fov(state, loc, 12, true);
            visible.contains(&player_loc)
        } else {
            false
        }
    }

    fn hunt_player(&mut self, my_id: usize, state: &mut GameState, game_objs: &mut GameObjects, my_loc: (i32, i32, i8)) {
        let player_loc = game_objs.player_location();
        
        // Am I within range of the player? (I don't have ranged monsters yet so just check if monster is adjacent to the player...)
        let in_range = util::are_adj(my_loc, player_loc);

        if in_range {
            self.plan.push_front(Action::Attack(player_loc));
        } else if self.can_see_player(state, my_loc, player_loc) {
            self.calc_plan_to_move(state, player_loc, true, my_loc);
        } else {
            let guess = self.best_guess_toward_player(state, my_loc, player_loc);
            self.calc_plan_to_move(state, guess, true, my_loc);
        }

        self.follow_plan(my_id, state, game_objs, my_loc);        
    }

    fn wander(&mut self, my_id: usize, state: &mut GameState, game_objs: &mut GameObjects, my_loc: (i32, i32, i8)) {
        let player_loc = game_objs.player_location();

        // Need to give the monster a check here vs the player's 'passive stealth'
        if self.can_see_player(state, my_loc, player_loc) {
            self.attitude = Attitude::Hostile;
            self.active = true;
            self.hunt_player(my_id, state, game_objs, my_loc);
            return;
        } 

        // Continue on its current amble, or pick a new square
        if self.plan.is_empty() {
            let mut rng = rand::thread_rng();
            // try a bunch of times to find a new plae to move to.
            for _ in 0..50 {
                let r = rng.gen_range(-10, 11);
                let c = rng.gen_range(-10, 11);
                let n = (my_loc.0 + r, my_loc.1 + c, my_loc.2);
                if state.map.contains_key(&n) && state.map[&n].passable_dry_land() {
                    self.calc_plan_to_move(state, n, false, my_loc);
                }
            }
        }

        self.follow_plan(my_id, state, game_objs, my_loc);
    }

    fn idle_monster(&mut self, my_id: usize, state: &mut GameState, game_objs: &mut GameObjects, my_loc: (i32, i32, i8)) {
        let player_loc = game_objs.player_location();

        // Need to give the monster a check here vs the player's 'passive stealth'
        if self.can_see_player(state, my_loc, player_loc) {
            self.attitude = Attitude::Hostile;
            self.active = true;
            self.hunt_player(my_id, state, game_objs, my_loc);
            return;
        }

        // just pick a random adjacent square
        self.random_adj_sq(game_objs, state, my_loc);
        self.follow_plan(my_id, state, game_objs, my_loc);
    }

    pub fn take_turn(&mut self, my_id: usize, state: &mut GameState, game_objs: &mut GameObjects, loc: (i32, i32, i8)) {
        let curr_behaviour = if self.active {
            self.active_behaviour
        } else {
            self.inactive_behaviour
        };

        match curr_behaviour {
            Behaviour::Hunt => {
                self.hunt_player(my_id, state, game_objs, loc);
            },
            Behaviour::Wander => {
                self.wander(my_id, state, game_objs, loc);
            },
            Behaviour::Idle => {
                self.idle_monster(my_id, state, game_objs, loc);
            },
            Behaviour::Guard(_) | Behaviour:: Defend(_) => panic!("These are not implemented yet!"),
        }

        if self.plan.is_empty() {
            let player_loc = game_objs.player_location();
            self.check_schedule(state, loc, player_loc);
        }        
    }

    pub fn talk_to(&mut self, state: &mut GameState, dialogue: &DialogueLibrary, my_loc: (i32, i32, i8)) -> String {
        if self.voice == "monster" {
            let s = format!("{} growls.", self.name.with_def_article().capitalize());
            return s;
        }

        let line = dialogue::parse_voice_line(&dialogue::pick_voice_line(dialogue, &self.voice, self.attitude), &state.world_info,
            &self.name, my_loc);
        if self.attitude == Attitude::Stranger {
            // Perhaps a charisma check to possibly jump straight to friendly?
            self.attitude = Attitude::Indifferent;
        }

        line
    }

    // At the moment, just using the voice to determine the name, although maybe
    // I'll later need a bit for anonymous vs named
    pub fn npc_name(&self, indef: bool) -> String {
        if self.voice != "monster" {
            self.name.clone()
        } else if indef {
            self.name.with_indef_article()
        } else {
            self.name.with_def_article()
        }
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

// This could be in a data file and maybe one day will be but for now the compiler will help me avoid stupid typos
// in basic monster definitions!
pub struct MonsterFactory {
    // AC, HP, ch, colour, mode, attack_mod, dmg_dice, dmg_die, dmg_bonus, level, attributes, xp_value,
    // active_behaviour, inactive_behaviour
    table: HashMap<String, (u8, u8, char, (u8, u8, u8), NPCPersonality, u8, u8, u8, u8, u8, u128, u32, bool, Behaviour, Behaviour)>, 
}

impl MonsterFactory {
    pub fn init() -> MonsterFactory {
        let mut mf = MonsterFactory { table: HashMap::new() };

        mf.table.insert(String::from("kobold"), (13, 7, 'k', display::DULL_RED, NPCPersonality::SimpleMonster, 4, 1, 4, 2, 1,
            MA_OPEN_DOORS | MA_UNLOCK_DOORS | MA_PACK_TACTICS, 4, false, Behaviour::Hunt, Behaviour::Idle));
        mf.table.insert(String::from("goblin"), (15, 7, 'o', display::GREEN, NPCPersonality::SimpleMonster, 4, 1, 6, 2, 1,
            MA_OPEN_DOORS | MA_UNLOCK_DOORS, 4, false, Behaviour::Hunt, Behaviour::Idle));
        mf.table.insert(String::from("zombie"), (11, 8, 'z', display::GREY, NPCPersonality::BasicUndead, 4, 1, 6, 2, 1,
            MA_OPEN_DOORS | MA_FEARLESS  | MA_UNDEAD, 5, false, Behaviour::Hunt, Behaviour::Wander));
        mf.table.insert(String::from("skeleton"), (12, 8, 's', display::WHITE, NPCPersonality::BasicUndead, 4, 1, 6, 2, 1,
            MA_OPEN_DOORS | MA_FEARLESS  | MA_UNDEAD | MA_RESIST_PIERCE | MA_RESIST_SLASH, 6, false, Behaviour::Hunt, Behaviour::Wander));
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

        let monster_name = name.clone();
        let sym = stats.2;
        let mut npc = NPC { name: String::from(name), ac: stats.0, curr_hp: stats.1, max_hp: stats.1, attitude: Attitude::Indifferent, facts_known: Vec::new(), home_id: 0, 
            plan: VecDeque::new(), voice: String::from("monster"), schedule: Vec::new(), mode: stats.4, attack_mod: stats.5, dmg_dice: stats.6, dmg_die: stats.7, 
            dmg_bonus: stats.8, edc: self.calc_dc(stats.9), attributes: stats.10, curr_loc: loc, alive: true, xp_value: stats.11, inventory: Vec::new(),
            active: stats.12, active_behaviour: stats.13, inactive_behaviour: stats.14,
        };

        let mut rng = rand::thread_rng();
        let amt = rng.gen_range(1, 6);
        let gold = GoldPile::make(game_objs, amt, loc);
        npc.inventory.push(gold);

        let monster = GameObject::new(game_objs.next_id(), &monster_name, loc, sym, stats.3, stats.3, 
            Some(npc), None , None, None, None, true);
		let obj_id = monster.object_id;
        game_objs.add(monster);
        game_objs.listeners.insert((obj_id, EventType::TakeTurn));
    }
}
