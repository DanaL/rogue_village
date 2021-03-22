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

use std::collections::{HashMap, HashSet, VecDeque};
use std::u128;
//use std::time::Instant;

use rand::thread_rng;
use rand::Rng;
use serde::{Serialize, Deserialize};

use super::{EventResponse, EventType, GameState, Status};

use crate::battle;
use crate::battle::DamageType;
use crate::dialogue;
use crate::dialogue::DialogueLibrary;
use crate::display;
use crate::game_obj::{Ability, GameObject, GameObjectBase, GameObjectDB, GameObjects, Person};
use crate::items::{GoldPile, Item};
use crate::map::{Tile, DoorState};
use crate::pathfinding::find_path;
use crate::util;
use crate::util::StringUtils;
use crate::fov;

// Some bitmasks for various monster attributes
pub const MA_OPEN_DOORS: u128       = 0x00000001;
pub const MA_UNLOCK_DOORS: u128     = 0x00000002;
pub const MA_WEAK_VENOMOUS: u128    = 0x00000004;
pub const MA_PACK_TACTICS: u128     = 0x00000008;
pub const MA_FEARLESS: u128         = 0x00000010;
pub const MA_UNDEAD: u128           = 0x00000020;
pub const MA_RESIST_SLASH: u128     = 0x00000040;
pub const MA_RESIST_PIERCE: u128    = 0x00000080;
pub const MA_WEBSLINGER: u128       = 0x00000100;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Venue {
    TownSquare,
    Tavern,
    Shrine,
    Favourite((i32, i32, i8)),
    Visit(i32),
    Home(usize),
    Market,
    Smithy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgendaItem {
    pub from: (u16, u16),
    pub to: (u16, u16),
    pub priority: u8,
    pub place: Venue,
    pub label: String,
}

impl AgendaItem {
    pub fn new(from: (u16, u16), to: (u16, u16), priority: u8, place: Venue, label: String) -> AgendaItem {
        AgendaItem { from, to, priority, place, label, }
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
    pub base_info: GameObjectBase,
    pub ac: u8,
	pub max_hp: u8,
	pub curr_hp: u8,
	pub attitude: Attitude,
    pub facts_known: Vec<usize>,
    pub home: Option<Venue>,
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
    pub alive: bool, // as in function, HPs > 0, not indication of undead status
    pub xp_value: u32,
    pub inventory: Vec<GameObjects>,
    pub active: bool,
    pub active_behaviour: Behaviour,
    pub inactive_behaviour: Behaviour,
    pub level: u8,
    pub last_inventory: u32,
    pub recently_saw_player: bool,
}

impl NPC {
    pub fn villager(name: String, location: (i32, i32, i8), home: Option<Venue>, voice: &str, game_obj_db: &mut GameObjectDB) -> GameObjects {      
        let npc = NPC { base_info: GameObjectBase::new(game_obj_db.next_id(), location, false, '@', display::LIGHT_GREY, 
            display::LIGHT_GREY, true, &name),ac: 10, curr_hp: 8, max_hp: 8, attitude: Attitude::Stranger, facts_known: Vec::new(), home, plan: VecDeque::new(), 
            voice: String::from(voice), schedule: Vec::new(), mode: NPCPersonality::Villager, attack_mod: 2, dmg_dice: 1, dmg_die: 3, dmg_bonus: 0, edc: 12,
            attributes: MA_OPEN_DOORS | MA_UNLOCK_DOORS, alive: true, xp_value: 0, inventory: Vec::new(), active: true, active_behaviour: Behaviour::Idle, 
            inactive_behaviour: Behaviour::Idle, level: 0, last_inventory: 0, recently_saw_player: false,
        };

		GameObjects::NPC(npc)
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

    // Select the current, highest priority agenda item from the schedule
    pub fn curr_agenda_item(&self, state: &GameState) -> Option<AgendaItem> {
        let ct = state.curr_time();
        let minutes = ct.0 * 60 + ct.1;

        let mut items: Vec<&AgendaItem> = self.schedule.iter()
            .filter(|i| i.from.0 * 60 + i.from.1 <= minutes && minutes <= i.to.0 * 60 + i.to.1)
            .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));

        if items.is_empty() {
            None
        } else {
            Some(items[0].clone())
        }
    }

    pub fn talk_to(&mut self, state: &mut GameState, dialogue: &DialogueLibrary, extra_info: &mut HashMap<String, String>) -> String {
        if self.voice == "monster" {
            let s = format!("{} growls.", self.base_info.name.with_def_article().capitalize());
            return s;
        }

        let context = if let Some(curr_agenda) = self.curr_agenda_item(state) {
            // Eventually maybe voice lines for every context?
            if curr_agenda.label == "working" {
                curr_agenda.label.to_string()
            } else if curr_agenda.label == "supper" || curr_agenda.label == "lunch" {
                extra_info.insert("#meal#".to_string(), curr_agenda.label);
                "".to_string()
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };

        let line = dialogue::parse_voice_line(&dialogue::pick_voice_line(dialogue, &self.voice, self.attitude, &context), &state,
            &self.base_info.name, self.get_loc(), extra_info);
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
            self.base_info.name.clone()
        } else if indef {
            self.base_info.name.with_indef_article()
        } else {
            self.base_info.name.with_def_article()
        }
    }

    fn death_msg(&self, assailant_id: usize) -> String {
        if assailant_id == 0 {
            format!("You kill {}!", self.npc_name(false))        
        } else {
            format!("{} dies!", self.npc_name(false).capitalize())        
        }
    }
}

impl GameObject for NPC {
    fn blocks(&self) -> bool {
        true
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        self.base_info.location
    }

    fn set_loc(&mut self, loc: (i32, i32, i8)) {
        self.base_info.location = loc;
    }

    fn get_fullname(&self) -> String {
        self.base_info.name.clone()
    }

    fn obj_id(&self) -> usize {
        self.base_info.object_id
    }

    fn get_tile(&self) -> Tile {
        Tile::Thing(self.base_info.lit_colour, self.base_info.unlit_colour, self.base_info.symbol)
    }

    fn hidden(&self) -> bool {
        self.base_info.hidden
    }

    fn hide(&mut self) {
        self.base_info.hidden = true;
    }
    fn reveal(&mut self) {
        self.base_info.hidden = false;
    }

    fn receive_event(&mut self, _event: EventType, _state: &mut GameState, _player_loc: (i32, i32, i8)) -> Option<EventResponse> {
        None
    }
}

impl Person for NPC {
    fn damaged(&mut self, state: &mut GameState, amount: u8, dmg_type: DamageType, assailant_id: usize, _assailant_name: &str) {
        let mut adjusted_dmg = amount;
        match dmg_type {
            DamageType::Slashing => if self.attributes & MA_RESIST_SLASH != 0 { adjusted_dmg /= 2; },
            DamageType::Piercing => if self.attributes & MA_RESIST_PIERCE != 0 { adjusted_dmg /= 2; },
            _ => { },
        }

        let curr_hp = self.curr_hp;

        if adjusted_dmg >= curr_hp {
            self.alive = false;
            state.write_msg_buff(&self.death_msg(assailant_id));
            
        } else {
            self.curr_hp -= adjusted_dmg;
        }
    }

    fn get_hp(&self) -> (u8, u8) {
        (self.curr_hp, self.max_hp)
    }

    fn add_hp(&mut self, state: &mut GameState, amt: u8) {
        self.curr_hp += amt;
        let s = format!("{} looks better!", self.npc_name(false).capitalize());
        state.write_msg_buff(&s);
    }

    // Not deaing with statuses for monsters yet...
    fn add_status(&mut self, _status: Status) { }
    fn remove_status(&mut self, _status: Status) { }

    // I'm not (yet) giving monsters individual stats yet, so for ability checks 
    // just use their effect dc
    fn ability_check(&self, _ability: Ability) -> u8 {
        let roll = rand::thread_rng().gen_range(1, 21) + self.edc;

        roll
    }
}

pub fn take_turn(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB) {  
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_id = npc.obj_id();
    let npc_loc = npc.get_loc();
    let npc_mode = npc.mode;
    let curr_behaviour = if npc.active {
        npc.active_behaviour
    } else {
        npc.inactive_behaviour
    };
    
    match curr_behaviour {
        Behaviour::Hunt => {
            hunt_player(npc_id, npc_loc, state, game_obj_db);
        },
        Behaviour::Wander => {
            wander(npc_id, state, game_obj_db, npc_loc);
        },
        Behaviour::Idle => {
            if npc_mode == NPCPersonality::Villager {
                villager_schedule(npc_id, state, game_obj_db, npc_loc);
                follow_plan(npc_id, state, game_obj_db);
            } else {
                idle_monster(npc_id, state, game_obj_db, npc_loc);
            }
        },
        Behaviour::Guard(_) | Behaviour:: Defend(_) => panic!("These are not implemented yet!"),
    }
}

fn wander(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB, npc_loc: (i32, i32, i8)) {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();

    // Need to give the monster a check here vs the player's 'passive stealth'
    if can_see_player(state, game_obj_db, npc_loc, player_loc, npc_id) {
        let npc = game_obj_db.npc(npc_id).unwrap();
        npc.attitude = Attitude::Hostile;
        npc.active = true;
        hunt_player(npc_id, npc_loc, state, game_obj_db);
        return;
    } 

    // Continue on its current amble, or pick a new square
    let npc = game_obj_db.npc(npc_id).unwrap();
    let no_plan = npc.plan.is_empty();
    if no_plan {
        let mut rng = rand::thread_rng();
        // try a bunch of times to find a new plae to move to.
        for _ in 0..50 {
            let r = rng.gen_range(-10, 11);
            let c = rng.gen_range(-10, 11);
            let n = (npc_loc.0 + r, npc_loc.1 + c, npc_loc.2);
            if state.map.contains_key(&n) && state.map[&n].passable_dry_land() {
                calc_plan_to_move(npc_id, state, game_obj_db, n, false);
            }
        }
    }

    follow_plan(npc_id, state, game_obj_db);
}

fn idle_monster(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB, npc_loc: (i32, i32, i8)) {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();

    // Need to give the monster a check here vs the player's 'passive stealth'
    if can_see_player(state, game_obj_db, npc_loc, player_loc, npc_id) {
        let npc = game_obj_db.npc(npc_id).unwrap();
        npc.attitude = Attitude::Hostile;
        npc.active = true;
        hunt_player(npc_id, npc_loc, state, game_obj_db);
        return;
    }

    // just pick a random adjacent square
    random_adj_sq(npc_id, state, game_obj_db, npc_loc);
    follow_plan(npc_id, state, game_obj_db);
}

fn hunt_player(npc_id: usize, npc_loc: (i32, i32, i8), state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let sees = can_see_player(state, game_obj_db, npc_loc, player_loc, npc_id);
    let adj = util::are_adj(npc_loc, player_loc);

    if special_move(npc_id, state, game_obj_db, player_loc, sees, adj) {
        return;
    }
    
    if adj {
        let npc = game_obj_db.npc(npc_id).unwrap();
        npc.plan.push_front(Action::Attack(player_loc));
    } else if sees {
        calc_plan_to_move(npc_id, state, game_obj_db, player_loc, true);
    } else {
        let guess = best_guess_toward_player(state, npc_loc, player_loc);
        calc_plan_to_move(npc_id, state, game_obj_db, guess, true);
    }

    follow_plan(npc_id, state, game_obj_db);     
}

fn open_door(loc: (i32, i32, i8), state: &mut GameState, npc_name: String) {
    let s = format!("{} opens the door.", npc_name);
    state.write_msg_buff(&s);
    state.map.insert(loc, Tile::Door(DoorState::Open));
}

fn close_door(loc: (i32, i32, i8), state: &mut GameState, game_obj_db: &mut GameObjectDB, npc_id: usize, npc_name: String) {
    if game_obj_db.blocking_obj_at(&loc) {
        state.write_msg_buff("Please don't stand in the doorway.");
        let npc = game_obj_db.npc(npc_id).unwrap();
        npc.plan.push_front(Action::CloseDoor(loc));
    } else {
        if let Tile::Door(DoorState::Open) = state.map[&loc] {
            let npc = game_obj_db.npc(npc_id).unwrap();
            if npc.attitude == Attitude::Stranger {
                state.write_msg_buff("The villager closes the door.");
            } else {
                let s = format!("{} closes the door.", npc_name);
                state.write_msg_buff(&s);
            }
            state.map.insert(loc, Tile::Door(DoorState::Closed));
        }
    }
}


fn follow_plan(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_name = npc.npc_name(false).capitalize();
    let action = npc.plan.pop_front();

    if let Some(action) = action {
        match action {
            Action::Move(loc) => try_to_move_to_loc(npc_id, loc, state, game_obj_db),
            Action::OpenDoor(loc) => open_door(loc, state, npc_name),
            Action::CloseDoor(loc) => close_door(loc, state, game_obj_db, npc_id, npc_name),
            Action::Attack(_loc) => {
                //battle::monster_attacks_player(state, self, npc_id, game_obj_db);
            },
        }
    }
}

fn try_to_move_to_loc(npc_id: usize, goal_loc: (i32, i32, i8), state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let blocking_object = game_obj_db.blocking_obj_at(&goal_loc);
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_loc = npc.get_loc();
    let npc_mode = npc.mode;
    let npc_name = npc.npc_name(false).capitalize();

    if goal_loc == npc_loc {
        println!("Hmm I'm trying to move to my own location...");
    }   
    if blocking_object {
        match npc_mode {
            NPCPersonality::Villager => { state.write_msg_buff("\"Excuse me.\""); }
            _ => { }

        }
        // if someone/something is blocking path, clear the current plan which should trigger 
        // creating a new plan
        npc.plan.clear();
    } else if state.map[&goal_loc] == Tile::Door(DoorState::Closed) {
        npc.plan.push_front(Action::Move(goal_loc));
        open_door(goal_loc, state, npc_name);
    } else {
        // Villagers will close doors after they pass through them
        if npc_mode == NPCPersonality::Villager {
            if let Tile::Door(DoorState::Open) = state.map[&npc_loc] {
                npc.plan.push_front(Action::CloseDoor(npc_loc));                
            }
        }

        super::take_step(state, game_obj_db, npc_id, npc_loc, goal_loc);
    }
}

fn villager_schedule(npc_id: usize, state: &GameState, game_obj_db: &mut GameObjectDB, npc_loc: (i32, i32, i8)) {
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_home_id = if let Some(Venue::Home(home_id)) = npc.home {
        home_id as i32
    } else {
        -1
    };

    if let Some(curr_item) = npc.curr_agenda_item(state) {
        check_agenda_item(npc_id, state, game_obj_db, &curr_item, npc_loc);
    } else {
        // The default behaviour is to go home if nothing on the agenda.
        let b = &state.world_info.town_buildings.as_ref().unwrap();
        
        if npc_home_id > 0 {
            if !in_location(state, npc_loc, &b.homes[npc_home_id as usize], true) {
                go_to_place(npc_id, state, game_obj_db, &b.homes[npc_home_id as usize]);
            } else {
                random_adj_sq(npc_id, state, game_obj_db, npc_loc);                    
            }
        } else {
            random_adj_sq(npc_id, state, game_obj_db, npc_loc);
        }
    } 
}

fn check_agenda_item(npc_id: usize, state: &GameState, game_obj_db: &mut GameObjectDB, item: &AgendaItem, npc_loc: (i32, i32, i8)) {        
    let venue =
        match item.place {
            Venue::Tavern => &state.world_info.town_buildings.as_ref().unwrap().tavern,
            Venue::Market => &state.world_info.town_buildings.as_ref().unwrap().market,
            Venue::Smithy => &state.world_info.town_buildings.as_ref().unwrap().smithy,
            Venue::TownSquare => &state.world_info.town_square,
            _ => panic!("Haven't implemented that venue yet!"),
        };

    if !venue.is_empty() && !in_location(state, npc_loc, &venue, true) {
        go_to_place(npc_id, state, game_obj_db, &venue);
    } else {
        random_adj_sq(npc_id, state, game_obj_db, npc_loc);
    }
}

// Generally, when I have an NPC go a building/place, I assume it doesn't matter too much if 
// they go to specific square inside it, so just pick any one of them.
fn go_to_place(npc_id: usize, state: &GameState, game_obj_db: &mut GameObjectDB, sqs: &HashSet<(i32, i32, i8)>) {
    let j = thread_rng().gen_range(0, &sqs.len());
    let goal_loc = &sqs.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
    calc_plan_to_move(npc_id, state, game_obj_db, *goal_loc, false);
}

// Quick, dirty guess of which adjacent, open square is closest to the player
fn best_guess_toward_player(state: &GameState, loc: (i32, i32, i8), player_loc: (i32, i32, i8)) -> (i32, i32, i8) {
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

fn random_adj_sq(npc_id: usize, state: &GameState , game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8)) {
    if thread_rng().gen_range(0.0, 1.0) < 0.33 {
        let j = thread_rng().gen_range(0, util::ADJ.len()) as usize;
        let d = util::ADJ[j];
        let adj = (loc.0 + d.0, loc.1 + d.1, loc.2);
        if !game_obj_db.blocking_obj_at(&adj) && state.map[&adj].passable_dry_land() {
            calc_plan_to_move(npc_id, state, game_obj_db, adj, false);
        }
    }
}

// I should be able to move calc_plan_to_move, try_to_move_to_loc, etc to generic
// places for all Villager types since they'll be pretty same-y. The differences
// will be in how NPCs set their plans/schedules. 
fn calc_plan_to_move(npc_id: usize, state: &GameState, game_obj_db: &mut GameObjectDB, goal: (i32, i32, i8), stop_before: bool) {
    let npc = game_obj_db.npc(npc_id).unwrap();
    npc.plan.clear();

    let mut passable = HashMap::new();
    passable.insert(Tile::Grass, 1.0);
    passable.insert(Tile::Dirt, 1.0);
    passable.insert(Tile::Tree, 1.0);
    passable.insert(Tile::Bridge, 1.0);
    passable.insert(Tile::Door(DoorState::Open), 1.0);
    passable.insert(Tile::Door(DoorState::Broken), 1.0);
    passable.insert(Tile::Gate(DoorState::Open), 1.0);
    passable.insert(Tile::Gate(DoorState::Broken), 1.0);
    if npc.attributes & MA_OPEN_DOORS > 0 {
        passable.insert(Tile::Door(DoorState::Closed), 2.0);
    }
    if npc.attributes & MA_UNLOCK_DOORS > 0 {
        passable.insert(Tile::Door(DoorState::Locked), 2.5);
    }
    passable.insert(Tile::StoneFloor, 1.0);
    passable.insert(Tile::Floor, 1.0);
    passable.insert(Tile::Trigger, 1.0);
    passable.insert(Tile::Rubble, 1.50);

    let npc_loc = npc.get_loc();
    let mut path = find_path(&state.map, Some(game_obj_db), stop_before, npc_loc.0, npc_loc.1, 
        npc_loc.2, goal.0, goal.1, 50, &passable);
    
    let npc = game_obj_db.npc(npc_id).unwrap();
    path.pop(); // first square in path is the start location
    while !path.is_empty() {
        let sq = path.pop().unwrap();
        npc.plan.push_back(Action::Move((sq.0, sq.1, npc_loc.2)));
    }
}

fn spin_webs(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), npc_name: String, difficulty: u8) {
    let s = format!("{} spins a web.", npc_name);
    state.write_msg_buff(&s);
    let mut web = Item::web(game_obj_db, difficulty);
    web.set_loc(loc);
    game_obj_db.add(web);

    for adj in util::ADJ.iter() {
        let adj_loc = (loc.0 + adj.0, loc.1 + adj.1, loc.2);
        if state.map[&adj_loc].passable() && rand::thread_rng().gen_range(0.0, 1.0) < 0.66 {
            let mut web = Item::web(game_obj_db, difficulty);
            web.set_loc(adj_loc);
            game_obj_db.add(web);
        }
    }        
}

fn special_move(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB, player_loc: (i32, i32, i8), sees_player: bool, adj: bool) -> bool {
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_loc = npc.get_loc();
    let npc_name = npc.npc_name(false).capitalize();
    let attributes = npc.attributes;
    let difficulty = npc.edc;
    
    if attributes & MA_WEBSLINGER > 0 && sees_player && !adj {
        let d = util::distance(npc_loc.0, npc_loc.1, player_loc.0, player_loc.1);
        if d < 5.0 && rand::thread_rng().gen_range(0.0, 1.0) < 0.33 {
            spin_webs(state, game_obj_db, player_loc, npc_name, difficulty);
            return true;
        }
    }

    false
}

fn can_see_player(state: &GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), player_loc: (i32, i32, i8), npc_id: usize) -> bool {
    let dr = loc.0 - player_loc.0;
    let dc = loc.1 - player_loc.1;
    let d = dr * dr + dc * dc;

    // This distance check may be premature optimization. If monster fov turns out to not be a bottleneck
    // I can ditch it. But my first ever attempt at a roguelike was in Python in 2002 and you had to be
    // careful about speed...
    if d < 169 {
        // Is the player within the monster's FOV? If they recently saw the player or pass a perception check
        // then they can see the player. Otherwise, not. If they player passes out of the FOV, flip the 
        // recently saw player bit so that they have to make a new perception check if they loose track
        // of player
        let visible_sqs = fov::calc_fov(state, loc, 12, true);
        let in_fov = visible_sqs.contains(&player_loc);
        if !in_fov {
            if let Some(GameObjects::NPC(npc)) = game_obj_db.get_mut(npc_id) {
                npc.recently_saw_player = false;
            }
            return false;
        }
        
        let (npc_level, recently_saw_player) = if let Some(GameObjects::NPC(npc)) = game_obj_db.get(npc_id) {
            (npc.level, npc.recently_saw_player)
        } else {
            (0, false)
        };

        if in_fov && recently_saw_player {
            return true;
        }

        let mut rng = rand::thread_rng();
        let percept = rng.gen_range(1, 21) + npc_level;
        let player_stealth = game_obj_db.player().unwrap().stealth_score;
        if percept >= player_stealth {
            if let Some(GameObjects::NPC(npc)) = game_obj_db.get_mut(npc_id) {
                npc.recently_saw_player = true;
            }
            return true;
        }

        false
    } else {
        if let Some(GameObjects::NPC(npc)) = game_obj_db.get_mut(npc_id) {
            npc.recently_saw_player = false;
        }
        false
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
    // AC, HP, ch, colour, behaviour, attack_mod, dmg_dice, dmg_die, dmg_bonus, level, attributes, xp_value, active,
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
        mf.table.insert(String::from("dire rat"), (13, 8, 'r', display::GREY, NPCPersonality::SimpleMonster, 4, 1, 4, 0, 1,
            MA_WEAK_VENOMOUS, 5, false, Behaviour::Hunt, Behaviour::Wander));
        mf.table.insert(String::from("giant spider"), (14, 24, 's', display::GREY, NPCPersonality::SimpleMonster, 6, 1, 8, 0, 3,
            MA_WEAK_VENOMOUS | MA_WEBSLINGER, 8, false, Behaviour::Hunt, Behaviour::Wander));
        mf
    }

    fn calc_dc(&self, level: u8) -> u8 {
        if level < 3 {
            12
        } else if level < 6 {
            13
        } else if level <  9 {
            14
        } else if level < 12 {
            15
        } else if level < 15 {
            16
        } else if level < 18 {
            17        
        } else {
            18
        }
    }

    pub fn add_monster(&self, name: &str, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB) {
        if !self.table.contains_key(name) {
            let s = format!("Unknown monster: {}!!", name);
            panic!(s);
        }

        let stats = self.table.get(name).unwrap();

        let sym = stats.2;
        let mut npc = NPC { base_info: GameObjectBase::new(game_obj_db.next_id(), loc, false, sym, stats.3,  stats.3, true, name),
            ac: stats.0, curr_hp: stats.1, max_hp: stats.1, attitude: Attitude::Indifferent, facts_known: Vec::new(), home: None, plan: VecDeque::new(), voice: String::from("monster"), 
            schedule: Vec::new(), mode: stats.4, attack_mod: stats.5, dmg_dice: stats.6, dmg_die: stats.7, dmg_bonus: stats.8, edc: self.calc_dc(stats.9), attributes: stats.10, 
            alive: true, xp_value: stats.11, inventory: Vec::new(), active: stats.12, active_behaviour: stats.13, inactive_behaviour: stats.14, level: stats.9, last_inventory: 0,
            recently_saw_player: false,
        };

        let mut rng = rand::thread_rng();
        let amt = rng.gen_range(1, 6);
        let gold = GoldPile::make(game_obj_db, amt, loc);
        npc.inventory.push(gold);

        let obj_id = npc.obj_id();
        game_obj_db.add(GameObjects::NPC(npc));
        game_obj_db.listeners.insert((obj_id, EventType::TakeTurn));
    }
}
