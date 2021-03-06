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
use std::fs;
use std::u128;
//use std::time::Instant;

use rand::thread_rng;
use rand::Rng;
use serde::{Serialize, Deserialize};

use super::{EventResponse, EventType, GameState, Message, Status};

use crate::battle;
use crate::battle::DamageType;
use crate::dialogue;
use crate::dialogue::DialogueLibrary;
use crate::display;
use crate::effects;
use crate::effects::{AB_CREATE_PHANTASM, HasStatuses};
use crate::game_obj::{Ability, GameObject, GameObjectBase, GameObjectDB, GameObjects, Person};
use crate::items::{GoldPile, Item};
use crate::map::{Tile, DoorState};
use crate::pathfinding::find_path;
use crate::util;
use crate::util::StringUtils;
use crate::fov;

// Loot categories from monsters.txt
pub const LOOT_NONE: u128       = 0x00000001;
pub const LOOT_PITTANCE: u128   = 0x00000002;
pub const LOOT_MINOR_GEAR: u128 = 0x00000004;
pub const LOOT_MINOR_ITEM: u128 = 0x00000008;

// Some bitmasks for various monster attributes
pub const MA_OPEN_DOORS: u128        = 0x00000001;
pub const MA_UNLOCK_DOORS: u128      = 0x00000002;
pub const MA_WEAK_VENOMOUS: u128     = 0x00000004;
pub const MA_PACK_TACTICS: u128      = 0x00000008;
pub const MA_FEARLESS: u128          = 0x00000010;
pub const MA_UNDEAD: u128            = 0x00000020;
pub const MA_RESIST_SLASH: u128      = 0x00000040;
pub const MA_RESIST_PIERCE: u128     = 0x00000080;
pub const MA_WEBSLINGER: u128        = 0x00000100;
pub const MA_MINOR_BLACK_MAGIC: u128 = 0x00000200;
pub const MA_MINOR_TRICKERY: u128    = 0x00000400;
pub const MA_ILLUSION: u128          = 0x00000800;
pub const MA_CONFUSION: u128         = 0x00001000;
pub const MA_LEAVE_CORPSE: u128      = 0x00002000;
pub const MA_PARALYZE: u128          = 0x00004000;
pub const MA_SMASH_DOORS: u128       = 0x00008000;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Pronouns {
    Masculine,
    Feminine,
    Neutral,
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
    UnlockDoor((i32, i32, i8)),
    SmashDoor((i32, i32, i8)),
    Attack((i32, i32, i8)),
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub enum NPCPersonality {
    Villager,
    SimpleMonster,
    BasicUndead,
    Plant,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Behaviour {
    Idle,
    Hunt,
    Wander,
    Guard((i32, i32, i8)),
    Defend(usize),
    Plant,
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
    pub size: u8,
    pub pronouns: Pronouns,
    pub rarity: u8,
    pub statuses: Vec<(Status, u32)>,
}

impl NPC {
    pub fn villager(name: String, location: (i32, i32, i8), home: Option<Venue>, voice: &str, game_obj_db: &mut GameObjectDB) -> GameObjects {            
        let npc = NPC { base_info: GameObjectBase::new(game_obj_db.next_id(), location, false, '@', display::LIGHT_GREY, 
            display::LIGHT_GREY, true, &name), ac: 10, curr_hp: 8, max_hp: 8, attitude: Attitude::Stranger, facts_known: Vec::new(), home, plan: VecDeque::new(), 
            voice: String::from(voice), schedule: Vec::new(), mode: NPCPersonality::Villager, attack_mod: 2, dmg_dice: 1, dmg_die: 3, dmg_bonus: 0, edc: 12,
            attributes: MA_OPEN_DOORS | MA_UNLOCK_DOORS, alive: true, xp_value: 0, inventory: Vec::new(), active: true, active_behaviour: Behaviour::Idle, 
            inactive_behaviour: Behaviour::Idle, level: 0, last_inventory: 0, recently_saw_player: false, size: 2, pronouns: pick_pronouns(), rarity: 0,
            statuses: Vec::new(),
        };

		GameObjects::NPC(npc)
    }
    
    pub fn phantasm(name: String, location: (i32, i32, i8), sym: char, colour: (u8, u8, u8), game_obj_db: &mut GameObjectDB) -> GameObjects {
        let phantasm = NPC { base_info: GameObjectBase::new(game_obj_db.next_id(), location, false, sym, colour, colour, true, &name), ac: 10, curr_hp: 0, max_hp: 0, 
            attitude: Attitude::Hostile, facts_known: Vec::new(), home: None, plan: VecDeque::new(), voice: String::from("monster"), schedule: Vec::new(), 
            mode: NPCPersonality::SimpleMonster, attack_mod: 0, dmg_dice: 0, dmg_die: 0, dmg_bonus: 0, edc: 10, attributes: MA_FEARLESS | MA_ILLUSION, alive: true, 
            xp_value: 0, inventory: Vec::new(), active: true, active_behaviour: Behaviour::Hunt, inactive_behaviour: Behaviour::Hunt, level: 0, last_inventory: 0, recently_saw_player: false, 
            size: 2, pronouns: pick_pronouns(), rarity: 0, statuses: Vec::new(),
        };

		GameObjects::NPC(phantasm)
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
                curr_agenda.label
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

    pub fn get_corpse(&self, game_obj_db: &mut GameObjectDB) -> Vec<GameObjects> {
        let mut pieces = Vec::new();
        if self.get_fullname() == "fungal growth" {
            for _ in 0..rand::thread_rng().gen_range(1, 4) {
                let mut m = Item::get_item(game_obj_db, "piece of mushroom").unwrap();
                m.set_loc(self.get_loc());
                pieces.push(m);
            }
        }
    
        pieces
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

    fn receive_event(&mut self, event: EventType, state: &mut GameState, _player_loc: (i32, i32, i8)) -> Option<EventResponse> {
        if let EventType::DeathOf(_) = event {
            if self.attributes & MA_ILLUSION > 0 {
                // Illusions go away when their creator dies (I'm assuming here the illusion will only be wired up to listen for the
                // death of the person who created it)
                let s = format!("{} vanishes in a puff of mist!", self.npc_name(false).capitalize());
                let msg = Message::new(self.base_info.object_id, self.get_loc(), &s, "");
                state.msg_queue.push_back(msg);
                self.alive = false;
            }
        }
        
        None
    }
}

impl Person for NPC {    
    fn damaged(&mut self, state: &mut GameState, amount: u8, dmg_type: DamageType, assailant_id: usize, _assailant_name: &str) {
        if self.attributes & MA_ILLUSION > 0 {
            if rand::thread_rng().gen_range(0.0, 1.0) <= 0.75 {
                let msg = Message::new(self.base_info.object_id, self.get_loc(), "Your weapon seems to pass right through them!", "");
                state.msg_queue.push_back(msg);
            } else {
                let s = format!("{} vanishes in a puff of mist!", self.npc_name(false).capitalize());
                let msg = Message::new(self.base_info.object_id, self.get_loc(), &s, "");
                state.msg_queue.push_back(msg);
                self.alive = false;
                state.queued_events.push_back((EventType::DeathOf(self.base_info.object_id), self.get_loc(), self.base_info.object_id, None));
            }
            return;
        }

        let mut adjusted_dmg = amount;
        match dmg_type {
            DamageType::Slashing => if self.attributes & MA_RESIST_SLASH != 0 { adjusted_dmg /= 2; },
            DamageType::Piercing => if self.attributes & MA_RESIST_PIERCE != 0 { adjusted_dmg /= 2; },
            DamageType::Cold => { 
                let s = format!("{} is frozen!", self.npc_name(false).capitalize());
                let msg = Message::new(self.base_info.object_id, self.get_loc(), &s, "You hear a gasp.");
                state.msg_queue.push_back(msg);
            },
            _ => { },
        }

        let curr_hp = self.curr_hp;

        if adjusted_dmg >= curr_hp {
            self.alive = false;
            let msg = Message::new(self.base_info.object_id, self.get_loc(), &self.death_msg(assailant_id), "You think you've landed a fatal blow!");
            state.msg_queue.push_back(msg);
            
            state.queued_events.push_back((EventType::DeathOf(self.base_info.object_id), self.get_loc(), self.base_info.object_id, None));
        } else {
            self.curr_hp -= adjusted_dmg;
        }
    }
    
    fn get_hp(&self) -> (u8, u8) {
        (self.curr_hp, self.max_hp)
    }

    fn add_hp(&mut self, _: &mut GameState, amt: u8) {
        self.curr_hp += amt;        
    }

    // I'm not (yet) giving monsters individual stats yet, so for ability checks 
    // just use their attack mod for now
    fn ability_check(&self, _ability: Ability) -> u8 {
        rand::thread_rng().gen_range(1, 21) + self.attack_mod
    }

    fn attributes(&self) -> u128 {
        self.attributes
    }

    fn size(&self) -> u8 {
        self.size
    }

    fn mark_dead(&mut self) {
        self.alive = false;
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn calc_ac(&mut self) {
        // Don't yet have any monster abilities that'll boost their AC
    }
}

impl HasStatuses for NPC {
    fn get_statuses(&mut self) -> Option<&mut Vec<(Status, u32)>> {
        Some(&mut self.statuses)
    }
}

fn pick_pronouns() -> Pronouns {
    let roll = rand::thread_rng().gen_range(0, 3);
    if roll == 0 {
        Pronouns::Masculine
    } else if roll == 1 {
        Pronouns::Feminine
    } else {
        Pronouns::Neutral
    }
}

pub fn take_turn(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB) {  
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_loc = npc.get_loc();
    let npc_mode = npc.mode;
    let curr_behaviour = if npc.active {
        npc.active_behaviour
    } else {
        npc.inactive_behaviour
    };
    
    match curr_behaviour {
        Behaviour::Hunt => hunt_player(npc_id, npc_loc, state, game_obj_db),
        Behaviour::Wander => wander(npc_id, state, game_obj_db, npc_loc),
        Behaviour::Idle => {
            if npc_mode == NPCPersonality::Villager {
                villager_schedule(npc_id, state, game_obj_db, npc_loc);
                follow_plan(npc_id, state, game_obj_db);
            } else {
                idle_monster(npc_id, state, game_obj_db, npc_loc);
            }
        },
        Behaviour::Plant => plant_behaviour(npc_id, state, game_obj_db, npc_loc),
        Behaviour::Guard(_) | Behaviour:: Defend(_) => panic!("These are not implemented yet!"),
    }
}

fn plant_behaviour(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB, npc_loc: (i32, i32, i8)) {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_dc = npc.edc;
    if util::are_adj(npc_loc, player_loc) {
        let s = format!("{} releases spores!", npc.npc_name(false).capitalize());
        state.msg_queue.push_back(Message::new(npc_id, npc_loc, &s, "Something releases spores into the air!"));
        battle::apply_weak_poison(state, 0, game_obj_db, npc_dc);
        battle::apply_confusion(state, 0, game_obj_db, npc_dc);
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

pub fn heard_noise(npc_id: usize, loc: (i32, i32, i8), state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let player_loc = game_obj_db.player().unwrap().get_loc();
    let npc = game_obj_db.npc(npc_id).unwrap();
    // I need to make a better way to differentiate between monsters and villagers
    if npc.voice == "monster" {
        npc.attitude = Attitude::Hostile;
        npc.active = true;
        let npc_loc = npc.get_loc();

        if !can_see_player(state, game_obj_db, npc_loc, player_loc, npc_id) {
            calc_plan_to_move(npc_id, state, game_obj_db, loc, false);
        }
    }
}

fn hunt_player(npc_id: usize, npc_loc: (i32, i32, i8), state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let sees = can_see_player(state, game_obj_db, npc_loc, player_loc, npc_id);
    let adj = util::are_adj(npc_loc, player_loc);

    if special_move(npc_id, state, game_obj_db, player_loc, sees, adj) {
        return;
    }
    
    let npc = game_obj_db.npc(npc_id).unwrap();    
    if adj {        
        npc.plan.push_front(Action::Attack(player_loc));
    } else if sees {
        calc_plan_to_move(npc_id, state, game_obj_db, player_loc, true);
    } else if npc.plan.is_empty() {
        let guess = best_guess_toward_player(state, npc_loc, player_loc);
        calc_plan_to_move(npc_id, state, game_obj_db, guess, true);
    }

    follow_plan(npc_id, state, game_obj_db);     
}

fn open_door(npc_id: usize, loc: (i32, i32, i8), npc_loc: (i32, i32, i8), state: &mut GameState, npc_name: String) {
    state.map.insert(loc, Tile::Door(DoorState::Open));
    let s = format!("{} opens the door.", npc_name);
    let msg = Message::new(npc_id, npc_loc, &s, "You hear a door open.");
    state.msg_queue.push_back(msg);
}

fn unlock_door(npc_id: usize, loc: (i32, i32, i8), npc_loc: (i32, i32, i8), state: &mut GameState, npc_name: String) {
    state.map.insert(loc, Tile::Door(DoorState::Closed));
    let s = format!("{} fiddles with the lock.", npc_name);
    let msg = Message::new(npc_id, npc_loc, &s, "Something fiddles with the lock.");
    state.msg_queue.push_back(msg);
}

fn smash_door(npc_id: usize, loc: (i32, i32, i8), npc_loc: (i32, i32, i8), state: &mut GameState, npc_name: String, game_obj_db: &mut GameObjectDB) {    
    let npc = game_obj_db.npc(npc_id).unwrap();
    if npc.ability_check(Ability::Str) > 17 {
        state.map.insert(loc, Tile::Door(DoorState::Broken));
        let s = format!("{} smashes down the door.", npc_name);
        let msg = Message::new(npc_id, npc_loc, &s, "Wham! You hear wood rending!");
        state.msg_queue.push_back(msg);
        npc.plan.push_front(Action::Move(loc));
    } else {
        let s = format!("{} slams into the door.", npc_name);
        let msg = Message::new(npc_id, npc_loc, &s, "Wham! Something slams against a door!");
        state.msg_queue.push_back(msg);
        npc.plan.push_front(Action::SmashDoor(loc));
    }
}

fn close_door(loc: (i32, i32, i8), state: &mut GameState, game_obj_db: &mut GameObjectDB, npc_id: usize, npc_loc: (i32, i32, i8), npc_name: String) {
    if game_obj_db.blocking_obj_at(&loc) {
        let msg = Message::new(npc_id, npc_loc, "\"Please don't stand in the doorway.\"", "\"Please don't stand in the doorway.\"");
        state.msg_queue.push_back(msg);
        let npc = game_obj_db.npc(npc_id).unwrap();
        npc.plan.push_front(Action::CloseDoor(loc));
    } else {
        if let Tile::Door(DoorState::Open) = state.map[&loc] {
            state.map.insert(loc, Tile::Door(DoorState::Closed));
            let npc = game_obj_db.npc(npc_id).unwrap();
            if npc.attitude == Attitude::Stranger {
                let msg = Message::new(npc_id, npc_loc, "The villager closes the door.", "You hear a door close.");
                state.msg_queue.push_back(msg);
            } else {
                let s = format!("{} closes the door.", npc_name);
                let msg = Message::new(npc_id, npc_loc, &s, "You hear a door close.");
                state.msg_queue.push_back(msg);                
            }            
        }
    }
}

fn follow_plan(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_loc = npc.get_loc();
    let npc_name = npc.npc_name(false).capitalize();
    let action = npc.plan.pop_front();

    if let Some(action) = action {
        match action {
            Action::Move(loc) => try_to_move_to_loc(npc_id, loc, state, game_obj_db),
            Action::OpenDoor(loc) => open_door(npc_id, loc, npc_loc, state, npc_name),
            Action::CloseDoor(loc) => close_door(loc, state, game_obj_db, npc_id, npc_loc, npc_name),
            Action::UnlockDoor(loc) => unlock_door(npc_id, loc, npc_loc, state, npc_name),
            Action::SmashDoor(loc) => smash_door(npc_id, loc, npc_loc, state, npc_name, game_obj_db),
            Action::Attack(_loc) => battle::monster_attacks_player(state, npc_id, game_obj_db),
        }
    }
}

fn try_to_move_to_loc(npc_id: usize, goal_loc: (i32, i32, i8), state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let blocking_object = game_obj_db.blocking_obj_at(&goal_loc);
    let npc = game_obj_db.npc(npc_id).unwrap();
    let attributes = npc.attributes;
    let npc_loc = npc.get_loc();
    let npc_mode = npc.mode;
    let npc_name = npc.npc_name(false).capitalize();

    if goal_loc == npc_loc {
        println!("Hmm I'm trying to move to my own location...");
    }   
    if blocking_object {
        if npc_mode == NPCPersonality::Villager {
            state.msg_queue.push_back(Message::new(npc_id, goal_loc, "\"Excuse me.\"", "\"Excuse me.\""));            
        }
        // if someone/something is blocking path, clear the current plan which should trigger 
        // creating a new plan
        npc.plan.clear();
    } else if state.map[&goal_loc] == Tile::Door(DoorState::Closed) {
        npc.plan.push_front(Action::Move(goal_loc));
        open_door(npc_id, goal_loc, npc_loc, state, npc_name);
    } else if state.map[&goal_loc] == Tile::Door(DoorState::Locked) && attributes & MA_UNLOCK_DOORS > 0 {
        npc.plan.push_front(Action::Move(goal_loc));
        unlock_door(npc_id, goal_loc, npc_loc, state, npc_name);
    } else if state.map[&goal_loc] == Tile::Door(DoorState::Locked) && attributes & MA_SMASH_DOORS > 0 {
        smash_door(npc_id, goal_loc, npc_loc, state, npc_name, game_obj_db);
    } else {
        // Villagers will close doors after they pass through them
        if npc_mode == NPCPersonality::Villager {
            if let Tile::Door(DoorState::Open) = state.map[&npc_loc] {
                npc.plan.push_front(Action::CloseDoor(npc_loc));                
            }
        }

        super::take_step(state, game_obj_db, npc_id, npc_loc, goal_loc, false);
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
    if npc.attributes & (MA_UNLOCK_DOORS | MA_SMASH_DOORS) > 0 {
        passable.insert(Tile::Door(DoorState::Locked), 2.5);
    }
    passable.insert(Tile::StoneFloor, 1.0);
    passable.insert(Tile::Floor, 1.0);
    passable.insert(Tile::Trigger, 1.0);
    
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

fn spin_webs(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), npc_id: usize, npc_name: String, difficulty: u8) {
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

    let s = format!("{} spins a web.", npc_name);
    state.msg_queue.push_back(Message::new(npc_id, loc, &s, ""));    
}

fn minor_black_magic(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB, player_loc: (i32, i32, i8), sees_player: bool, _adj: bool) -> bool {
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_loc = npc.get_loc();
    let npc_name = npc.npc_name(false);
    let distance = util::distance(npc_loc.0, npc_loc.1, player_loc.0, player_loc.1);
    let npc_hp = npc.get_hp();
    
    // if they are injured and near the player, they will blink away 50% of the time    
    if  (npc_hp.0 as f32 / npc_hp.1 as f32) < 0.33 && distance <= 3.0 && rand::thread_rng().gen_range(0.0, 1.0) < 0.5 {
        let s = format!("{} blinks away!", npc_name.capitalize());
        state.msg_queue.push_back(Message::new(npc_id, npc_loc, &s, "You hear a poof."));
        effects::apply_effects(state, npc_id, game_obj_db, effects::EF_BLINK);
        return true;
    }

    if sees_player && distance <= 3.0 && rand::thread_rng().gen_range(0.0, 1.0) < 0.33 {
        let s = format!("{} mumbles.", npc_name.capitalize());
        state.msg_queue.push_back(Message::new(npc_id, npc_loc, &s, "You hear mumbling."));
        state.msg_queue.push_back(Message::new(npc_id, player_loc, "A shroud falls over your eyes!", "A shroud falls over your eyes!"));
        let player = game_obj_db.player().unwrap();
        effects::add_status(player, Status::Blind, state.turn + rand::thread_rng().gen_range(3, 6));
        return true;
    }

    if sees_player && distance <= 3.0 && rand::thread_rng().gen_range(0.0, 1.0) < 0.33 {
        let s = format!("{} mumbles.", npc_name.capitalize());
        state.msg_queue.push_back(Message::new(npc_id, npc_loc, &s, "You hear mumbling."));
        state.msg_queue.push_back(Message::new(npc_id, npc_loc, "You have been cursed!", "You have been cursed!"));
        let player = game_obj_db.player().unwrap();
        effects::add_status(player, Status::Bane, state.turn + rand::thread_rng().gen_range(3, 6));
        return true;
    }

    false
}

fn create_phantasm(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB, centre: (i32, i32, i8)) {
    let mut options = Vec::new();
    for adj in util::ADJ.iter() {
        let loc = (centre.0 + adj.0, centre.1 + adj.1, centre.2);
        if !game_obj_db.location_occupied(&loc) && state.map[&loc].passable() {
            options.push(loc);
        }
    }

    if !options.is_empty() {
        let j = rand::thread_rng().gen_range(0, options.len());
        let npc = game_obj_db.npc(npc_id).unwrap();
        let ch = npc.base_info.symbol;
        let colour = npc.base_info.lit_colour;
        let name = &npc.base_info.name.to_string();
        let phantasm_loc = options[j];
        let phantasm = NPC::phantasm(name.to_string(), phantasm_loc, ch, colour, game_obj_db);
        let pid = phantasm.obj_id();
        
        game_obj_db.add(phantasm);
        game_obj_db.listeners.insert((pid, EventType::TakeTurn));
        game_obj_db.listeners.insert((pid, EventType::DeathOf(npc_id)));

        let npc = game_obj_db.npc(pid).unwrap();
        effects::add_status(npc, Status::FadeAfter, state.turn + 10);

        let s = format!("Another {} appears!", name);
        state.msg_queue.push_back(Message::new(pid, phantasm_loc, &s, ""));
        
        // The caster sometimes swaps places with the newly summoned phantasm
        if rand::thread_rng().gen_range(0.0, 1.0) < 0.33 {
            let npc = game_obj_db.get_mut(npc_id).unwrap();
            let npc_loc = npc.get_loc();
            game_obj_db.set_to_loc(npc_id, phantasm_loc);
            game_obj_db.set_to_loc(pid, npc_loc);
        }
    }
}

fn minor_trickery(npc_id: usize, state: &mut GameState, game_obj_db: &mut GameObjectDB, player_loc: (i32, i32, i8), sees_player: bool, adj: bool) -> bool {
    let npc = game_obj_db.npc(npc_id).unwrap();
    let npc_loc = npc.get_loc();
    let npc_hp = npc.get_hp();
    let npc_name = npc.npc_name(false);
    let distance = util::distance(npc_loc.0, npc_loc.1, player_loc.0, player_loc.1);

    let mut invisible = false;
    let mut cast_phantasm = false;
    for status in npc.get_statuses().unwrap().iter() {
        if status.0 == Status::Invisible {
            invisible = true;
            break;
        }
        if status.0 == Status::CoolingDown(AB_CREATE_PHANTASM) {
            cast_phantasm = true;
            break;
        }
    }

    // if they are injured and near the player, they will blink away 50% of the time (this check is cut-n-pasted from minor_black_magic...)
    if  (npc_hp.0 as f32 / npc_hp.1 as f32) < 0.33 && distance <= 3.0 && rand::thread_rng().gen_range(0.0, 1.0) < 0.5 {
        let s = format!("{} blinks away!", npc_name.capitalize());
        state.msg_queue.push_back(Message::new(npc_id, npc_loc, &s, "You hear a poof."));
        effects::apply_effects(state, npc_id, game_obj_db, effects::EF_BLINK);
        return true;
    }

    if sees_player && !invisible && rand::thread_rng().gen_range(0.0, 1.0) < 0.33 {
        let s = format!("{} disappears!", npc_name.capitalize());
        state.msg_queue.push_back(Message::new(npc_id, npc_loc, &s, ""));
        effects::add_status(npc, Status::Invisible, state.turn + rand::thread_rng().gen_range(5, 8));
        return true;
    }

    if !cast_phantasm && adj && !invisible && rand::thread_rng().gen_range(0.0, 1.0) < 0.33 {
        // create three phantasm duplicates
        create_phantasm(npc_id, state, game_obj_db, player_loc);
        create_phantasm(npc_id, state, game_obj_db, player_loc);
        create_phantasm(npc_id, state, game_obj_db, player_loc);

        let npc = game_obj_db.npc(npc_id).unwrap();
        effects::add_status(npc, Status::CoolingDown(AB_CREATE_PHANTASM), state.turn + 10);
        return true;
    }

    false
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
            spin_webs(state, game_obj_db, player_loc, npc_id, npc_name, difficulty);
            return true;
        }
    }

    if attributes & MA_MINOR_BLACK_MAGIC > 0 && minor_black_magic(npc_id, state, game_obj_db, player_loc, sees_player, adj) {
        return true;        
    }

    if attributes & MA_MINOR_TRICKERY > 0 && minor_trickery(npc_id, state, game_obj_db, player_loc, sees_player, adj) {
        return true;
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
        let indoor_sqs = sqs.iter()
                            .filter(|sq| state.map[&sq].indoors())
                            .collect::<HashSet<&(i32, i32, i8)>>();
                                          
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

pub struct MonsterFactory {
    // AC, HP, ch, colour, behaviour, attack_mod, dmg_dice, dmg_die, dmg_bonus, level, attributes, xp_value, active,
    // active_behaviour, inactive_behaviour, size,
    table: HashMap<String, (u8, u8, char, (u8, u8, u8), NPCPersonality, u8, u8, u8, u8, u8, u128, u32, bool, Behaviour, Behaviour, u8, u8, u128)>,
    index_by_lvl: HashMap<u8, Vec<String>>,
}

impl MonsterFactory {
    fn to_personality(text: &str) -> NPCPersonality {
        match text {
            "SimpleMonster" => NPCPersonality::SimpleMonster,
            "BasicUndead" => NPCPersonality::BasicUndead,
            "Plant" => NPCPersonality::Plant,
            _ => {
                panic!("{}", format!("Unknown personality: {}", text));
            }
        }
    }

    fn to_colour(text: &str) -> (u8, u8, u8) {
        match text {    
            "BEIGE" => display::BEIGE,        
            "BLACK" => display::BLACK,
            "BLUE" => display::BLUE,
            "BRIGHT_RED" => display::BRIGHT_RED,
            "BROWN" => display::BROWN,
            "DARK_BLUE" => display::DARK_BROWN,
            "DARK_BROWN" => display::DARK_BROWN,
            "DARK_GREEN" => display::DARK_GREEN,
            "DARK_GREY" => display::DARK_GREY,
            "DULL_RED" => display::DULL_RED,
            "GOLD" => display::GOLD,
            "GREEN" => display::GREEN,
            "GREY" => display::GREY,
            "LIGHT_BLUE" => display::LIGHT_BLUE,
            "LIGHT_BROWN" => display::LIGHT_BROWN,
            "LIGHT_GREY" => display::LIGHT_GREY,
            "PINK" => display::PINK,
            "PURPLE" => display::PURPLE,
            "WHITE" => display::WHITE,
            "YELLOW" => display::YELLOW,
            "YELLOW_ORANGE" => display::YELLOW_ORANGE,
            _ => {
                panic!("{}", format!("Unknown colour: {}!", text));
            }
        }
    }

    fn to_behaviour(text: &str) -> Behaviour {
        match text {
            "hunt" => Behaviour::Hunt,
            "idle" => Behaviour::Idle,
            "wander" => Behaviour::Wander,
            "plant" => Behaviour::Plant,
            _ => {
                panic!("{}", format!("Unknown behaviour: {}!", text));
            }
        }
    }

    fn parse_loot_field(text: &str) -> u128 {
        let mut loot = 0;

        let fields = text.split('|').map(|l| l.trim()).collect::<Vec<&str>>();
        for field in fields {
            loot |= match field {
                "NONE" => LOOT_NONE,
                "PITTANCE" => LOOT_PITTANCE,
                "MINOR_GEAR" => LOOT_MINOR_GEAR,
                "MINOR_ITEM" => LOOT_MINOR_ITEM,
                _ => {
                    panic!("{}", format!("Unknown loot type: {}", field));
                }
            }
        }
        loot
    }

    fn parse_attributes(text: &str) -> u128 {
        let mut attributes = 0;

        let attrs = text.split('|').map(|a| a.trim()).collect::<Vec<&str>>();
        for a in attrs {
            attributes |= match a {
                "MA_OPEN_DOORS" => MA_OPEN_DOORS,
                "MA_UNLOCK_DOORS" => MA_UNLOCK_DOORS,
                "MA_PACK_TACTICS" => MA_PACK_TACTICS,
                "MA_FEARLESS" => MA_FEARLESS,
                "MA_UNDEAD" => MA_UNDEAD,
                "MA_RESIST_PIERCE" => MA_RESIST_PIERCE,
                "MA_RESIST_SLASH" => MA_RESIST_PIERCE,
                "MA_WEAK_VENOMOUS" => MA_WEAK_VENOMOUS,
                "MA_WEBSLINGER" => MA_WEBSLINGER,
                "MA_MINOR_BLACK_MAGIC" => MA_MINOR_BLACK_MAGIC,
                "MA_MINOR_TRICKERY" => MA_MINOR_TRICKERY,
                "MA_LEAVE_CORPSE" => MA_LEAVE_CORPSE,
                "MA_PARALYZE" => MA_PARALYZE,
                "MA_SMASH_DOORS" => MA_SMASH_DOORS,
                "SPORES" => {
                    let roll = rand::thread_rng().gen_range(0.0, 1.0);
                    if roll < 0.4 {
                        MA_WEAK_VENOMOUS
                    } else if roll < 0.8 {
                        MA_CONFUSION
                    } else {
                        MA_WEAK_VENOMOUS | MA_CONFUSION
                    }
                },
                "NONE" => 0,
                _ => {
                    panic!("{}", format!("Unknown attribute: {}!", a));
                }
            }
        }
        attributes
    }

    fn parse_line(line: &str) -> (String, (u8, u8, char, (u8, u8, u8), NPCPersonality, u8, u8, u8, u8, u8, u128, u32, bool, Behaviour, Behaviour, u8, u8, u128)) {
        let pieces = line.split(',').collect::<Vec<&str>>();
        let name = pieces[0].trim();
        let level = pieces[1].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let ac = pieces[2].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let hp = pieces[3].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let ch = pieces[4].trim().chars().next().unwrap();
        let colour = MonsterFactory::to_colour(pieces[5].trim());
        let personality = MonsterFactory::to_personality(pieces[6].trim());
        let attack_mod = pieces[7].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let dmg_dice = pieces[8].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let dmg_die = pieces[9].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let dmg_bonus = pieces[10].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let xp_value = pieces[11].trim().parse::<u32>().expect("Incorrectly formatted line in monster file!");
        let active_behaviour = MonsterFactory::to_behaviour(pieces[12].trim());
        let inactive_behaviour = MonsterFactory::to_behaviour(pieces[13].trim());
        let size = pieces[14].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let rarity = pieces[15].trim().parse::<u8>().expect("Incorrectly formatted line in monster file!");
        let loot = MonsterFactory::parse_loot_field(pieces[16]);
        let attributes = MonsterFactory::parse_attributes(pieces[17]);

        (name.to_string(), (ac, hp, ch, colour, personality, attack_mod, dmg_dice, dmg_die, dmg_bonus, level, attributes, xp_value, false, active_behaviour, inactive_behaviour, size, rarity, loot))
    }

    pub fn init() -> MonsterFactory {
        let mut mf = MonsterFactory { table: HashMap::new(), index_by_lvl: HashMap::new(), };

        let contents = fs::read_to_string("monsters.txt")
            .expect("Unable to find building templates file!");
        let lines = contents.split('\n').collect::<Vec<&str>>();
        for line in lines.iter().skip(1) {
            let entry = MonsterFactory::parse_line(line);

            let name = entry.0;
            let level = entry.1.9;
            mf.table.insert(name.clone(), entry.1);

            mf.index_by_lvl
              .entry(level)
              .or_insert(Vec::new())
              .push(name.clone());
        }

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

    fn set_loot(&self, loot_fields: u128, game_obj_db: &mut GameObjectDB) -> Vec<GameObjects> {
        let mut rng = rand::thread_rng();
        let mut items = Vec::new();

        if loot_fields & LOOT_PITTANCE > 0 && rng.gen_range(0.0, 1.0) < 0.33 {   
            let amt = rng.gen_range(3, 6);
            let gold = GoldPile::make(game_obj_db, amt, (-1, -1, -1));
            items.push(gold);            
        }

        if loot_fields & LOOT_MINOR_GEAR > 0 {
            if rng.gen_range(0.0, 1.0) < 0.1 {
                for _ in 3..6 {
                    items.push(Item::get_item(game_obj_db, "arrow").unwrap());
                }
            }
            if rng.gen_range(0.0, 1.0) < 0.1 {
                items.push(Item::get_item(game_obj_db, "shortsword").unwrap());
            }

        }

        if loot_fields & LOOT_MINOR_ITEM > 0 && rng.gen_range(0.0, 1.0) < 0.5 {
            if rng.gen_range(0.0, 1.0) < 0.5 {
                items.push(Item::get_item(game_obj_db, "potion of healing").unwrap());
            } else {
                items.push(Item::get_item(game_obj_db, "scroll of blink").unwrap());
            }
        }

        items
    }

    pub fn monster(&self, name: &str, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB) {
        if !self.table.contains_key(name) {
            panic!("{}", format!("Unknown monster: {}!!", name));
        }

        let stats = self.table.get(name).unwrap();

        let sym = stats.2;
        let mut npc = NPC { base_info: GameObjectBase::new(game_obj_db.next_id(), loc, false, sym, stats.3,  stats.3, true, name),
            ac: stats.0, curr_hp: stats.1, max_hp: stats.1, attitude: Attitude::Indifferent, facts_known: Vec::new(), home: None, plan: VecDeque::new(), voice: String::from("monster"), 
            schedule: Vec::new(), mode: stats.4, attack_mod: stats.5, dmg_dice: stats.6, dmg_die: stats.7, dmg_bonus: stats.8, edc: self.calc_dc(stats.9), attributes: stats.10, 
            alive: true, xp_value: stats.11, inventory: Vec::new(), active: stats.12, active_behaviour: stats.13, inactive_behaviour: stats.14, level: stats.9, last_inventory: 0,
            recently_saw_player: false, size: stats.15, pronouns: pick_pronouns(), rarity: stats.16, statuses: Vec::new(),
        };

        let items = self.set_loot(stats.17, game_obj_db);
        for item in items {
            npc.inventory.push(item);
        }

        let obj_id = GameObject::obj_id(&npc);
        game_obj_db.add(GameObjects::NPC(npc));
        game_obj_db.listeners.insert((obj_id, EventType::TakeTurn));
    }

    pub fn monster_for_dungeon(&self, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB) {
        let monster_level = MonsterFactory::rnd_monster_level(loc.2 as u8);
        let options = self.index_by_lvl[&monster_level].len();
        let choice = rand::thread_rng().gen_range(0, options);
        let name = &self.index_by_lvl[&monster_level][choice];
        self.monster(name, loc, game_obj_db);
    }

    fn rnd_monster_level(dungeon_level: u8) -> u8 {
        if dungeon_level == 1 {
            return 1;
        }

        let mut guass = util::general_guassian(dungeon_level as f32, 1.15).round();
        if guass < 1.0 {
            guass = 1.0;
        }

        if dungeon_level > 3 && guass < dungeon_level as f32 - 3.0 {
            guass = dungeon_level as f32 - 3.0;
        }

        if guass > 4.0 {
            guass = 4.0;
        }

        guass as u8
    }
}
