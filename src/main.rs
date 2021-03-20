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

#![allow(dead_code)]

extern crate rand;
extern crate sdl2;
extern crate serde;

mod actor;
mod battle;
mod dialogue;
mod display;
mod dungeon;
mod effects;
mod game_obj;
mod fov;
mod items;
mod map;
mod pathfinding;
mod player;
mod shops;
mod town;
mod util;
mod wilderness;
mod world;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::prelude::*;
use std::fs;
use std::fs::File;
//use std::time::Duration;
use std::path::Path;

use std::time::Instant;

use rand::{Rng, prelude::SliceRandom, thread_rng};
use serde::{Serialize, Deserialize};

use actor::{Attitude, MonsterFactory, Venue};
use dialogue::DialogueLibrary;
use display::{GameUI, SidebarInfo, WHITE};
use game_obj::{GameObject, GameObjectDB, GameObjects};
use items::{GoldPile, IA_CONSUMABLE, ItemType};
use map::{DoorState, ShrineType, Tile};
use player::{Ability, Player};
use util::StringUtils;
use world::WorldInfo;

const MSG_HISTORY_LENGTH: usize = 50;
const FOV_WIDTH: usize = 41;
const FOV_HEIGHT: usize = 21;
const PLAYER_INV: (i32, i32, i8) = (-999, -999, -128);

pub type Map = HashMap<(i32, i32, i8), map::Tile>;

enum ExitReason {
    Save,
    Win,
    Quit,
    Death(String),
}

pub struct EventResponse {
    object_id: usize,
    event_type: EventType,
}

impl EventResponse {
    pub fn new(object_id: usize, event_type: EventType) -> EventResponse {
        EventResponse { object_id, event_type, }
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub enum EventType {
    EndOfTurn,
    Update,
    LightExpired,
    TakeTurn,
    SteppedOn,
    Triggered, // used in chains of events. Ie., a tigger is stepped on and it sends a Triggered event to the gate it controls
    LitUp,
    GateOpened,
    GateClosed,
    PlayerKilled,
    LevelUp,
    TrapRevealed,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Status {
    PassUntil(u32),
    RestAtInn(u32),
}

pub enum Cmd {
    Quit,
    Move(String),
    MsgHistory,
    PickUp,
    ShowInventory,
    DropItem,
    ShowCharacterSheet,
    ToggleEquipment,
    Pass,
    Open((i32, i32, i8)),
    Close((i32, i32, i8)),
    Quaff,
    Search,
    Eat,
    Save,
    Chat((i32, i32, i8)),
    Use,
    Help,
    Down,
    Up,
    WizardCommand,
}

#[derive(Serialize, Deserialize)]
pub struct GameState {
    msg_buff: VecDeque<String>,
    msg_history: VecDeque<(String, u32)>,
    map: Map,
    turn: u32,
    world_info: WorldInfo,
    tile_memory: HashMap<(i32, i32, i8), Tile>,
    lit_sqs: HashSet<(i32, i32, i8)>, // by light sources independent of player
    aura_sqs: HashSet<(i32, i32, i8)>, // areas of special effects
    queued_events: VecDeque<(EventType, (i32, i32, i8), usize, Option<String>)>, // events queue during a turn that should be resolved at the end of turn
}

impl GameState {
    pub fn init(map: Map, world_info: WorldInfo) -> GameState {
        GameState {
            msg_buff: VecDeque::new(),
            msg_history: VecDeque::new(),
            map,
            turn: 0,
            world_info: world_info,
            tile_memory: HashMap::new(),
            lit_sqs: HashSet::new(),
            aura_sqs: HashSet::new(),
            queued_events: VecDeque::new(),
        }
    }

    pub fn add_to_msg_history(&mut self, msg: &str) {
        if self.msg_history.is_empty() || msg != self.msg_history[0].0 {
            self.msg_history.push_front((String::from(msg), 1));
        } else {
            self.msg_history[0].1 += 1;
        }

        if self.msg_history.len() > MSG_HISTORY_LENGTH {
            self.msg_history.pop_back();
        }
    }

    pub fn write_msg_buff(&mut self, msg: &str) {
        let s = String::from(msg);
        self.msg_buff.push_back(s);

        if !msg.is_empty() {
            self.add_to_msg_history(msg);
        }
    }

    pub fn curr_sidebar_info(&self, game_obj_db: &mut GameObjectDB) -> SidebarInfo {
        let player = game_obj_db.player().unwrap();
        let loc = player.get_loc();
        let weapon_name = match player.readied_weapon() {
            Some(res) => res.1.capitalize(),
            None => "Empty handed".to_string(),
        };
        
        SidebarInfo::new(player.get_fullname(), player.curr_hp, player.max_hp, self.turn, player.ac,
        player.purse, weapon_name, loc.2 as u8)
    }

    // I made life difficult for myself by deciding that Turn 0 of the game is 8:00am T_T
    // 1 turn is 10 seconds (setting aside all concerns about realize and how the amount of stuff one
    // can do in 10 seconds will in no way correspond to one action in the game...) so an hour is 
    // 360 turns
    pub fn curr_time(&self) -> (u16, u16) {
        let normalized = (self.turn + 2880) % 8640; // 8640 turns per day
        let hour = normalized / 360;
        let leftover = normalized - (hour * 360);
        let minute = leftover / 6;
        
        (hour as u16, minute as u16)
    }
}

fn show_message_history(state: &GameState, gui: &mut GameUI) {
    let mut lines = Vec::new();
    lines.push("".to_string());
    for j in 0..state.msg_history.len() {
        let mut s = state.msg_history[j].0.to_string();
        if state.msg_history[j].1 > 1 {
            s.push_str(" (x");
            s.push_str(&state.msg_history[j].1.to_string());
            s.push(')');
        }
        lines.push(s);
    }

    // Somedays I think rust is growing on me and some days I have to do stuff
    // like this so that I can pass an Vec of &strs to a function. I just didn't
    // want to have to constantly be typing .to_string() or String::From() when
    // calling write_long_msg() T_T
    let line_refs: Vec<&str> = lines.iter().map(AsRef::as_ref).collect();

    gui.write_long_msg(&line_refs, true);
}

fn title_screen(gui: &mut GameUI) {
    let mut lines = vec!["Welcome to Rogue Village 0.0.1!", ""];
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("");
    lines.push("Rogue Village is copyright 2021 by Dana Larose, see COPYING for licence info.");
    
    gui.write_long_msg(&lines, true);
}

fn calc_save_filename(player_name: &str) -> String {
    let s: String = player_name.chars()
        .map(|ch| match ch {
            'a'..='z' => ch,
            'A'..='Z' => ch,
            '0'..='9' => ch,
            _ => '_'
        }).collect();
    
    format!("{}.yaml", s)
}

fn serialize_game_data(state: &GameState, game_obj_db: &GameObjectDB) {
    let player_name = game_obj_db.get(0).unwrap().get_fullname();
    let game_data = (state, game_obj_db);
    let serialized = serde_yaml::to_string(&game_data).unwrap();
    let filename = calc_save_filename(&player_name);

    match File::create(&filename) {
        Ok(mut buffer) => {
            match buffer.write_all(serialized.as_bytes()) {
                Ok(_) => { },
                Err(_) => panic!("Oh no cannot write to file!"),
            }
        },
        Err(_) => panic!("Oh no file error!"),
    }
}

fn existing_save_file(player_name: &str) -> bool {
    let save_filename = calc_save_filename(player_name);

    let paths = fs::read_dir("./").unwrap();
    for path in paths {
        if save_filename == path.unwrap().path().file_name().unwrap().to_str().unwrap() {
            return true;
        }
    }

    false
}

fn load_save_game(player_name: &str) -> Result<(GameState, GameObjectDB), serde_yaml::Error> {
    let filename = calc_save_filename(player_name);
    let blob = fs::read_to_string(filename).expect("Error reading save file");
    let game_data: (GameState, GameObjectDB) = serde_yaml::from_str(&blob)?;
    
    Ok((game_data.0, game_data.1))
}

fn fetch_saved_data(player_name: &str) -> Option<(GameState, GameObjectDB)> {    
    match load_save_game(player_name) {
        Ok(gd) => Some(gd),
        Err(err) => { println!("error in save file {:?}", err); None },
    }
}

fn save_and_exit(state: &GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> Result<(), ExitReason> {
    let sbi = state.curr_sidebar_info(game_obj_db);
    match gui.query_yes_no("Save and exit? (y/n)", Some(&sbi)) {
        'y' => {
            serialize_game_data(state, game_obj_db);
            Err(ExitReason::Save)
        },
        _ => Ok(()),
    }
}

fn who_are_you(gui: &mut GameUI) -> String {
    loop {
        if let Some(name) = gui.query_user("Who are you?", 15, None) {
            if !name.trim().is_empty() {
                return name.trim().to_string();
            }
        }
    }
}

fn start_new_game(state: &GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI, player_name: String) {
    let mut menu = vec!["Welcome adventurer, please choose your role in RogueVillage:"];
    menu.push("");
    menu.push("  (a) Human Warrior - a doughty fighter who lives by the sword and...well");
    menu.push("                      hopefully just that first part.");
    menu.push("");
    menu.push("  (b) Human Rogue - a quick, sly adventurer who gets by on their light step");
    menu.push("                    and fast blade.");
    
    let answers: HashSet<&char> = ['a', 'b'].iter().collect();
    if let Some(answer) = gui.menu_wordy_picker(&menu, &answers) {
        if answer == 'a' {
            Player::new_warrior(game_obj_db, &player_name);
        } else {
            Player::new_warrior(game_obj_db, &player_name);
            //Player::new_rogue(game_obj_db, player_name);
        }

        game_obj_db.set_to_loc(0, pick_player_start_loc(&state));
    }
}

fn drop_zorkmids(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {
    let player = game_obj_db.player().unwrap();
    let player_loc = player.get_loc();
    let mut purse = player.purse;
    
    if purse == 0 {
        state.write_msg_buff("You have no money!");
        return 0.0;
    }
    
    let sbi = state.curr_sidebar_info(game_obj_db);
    let amt = gui.query_natural_num("How much?", Some(&sbi)).unwrap();
    if amt == 0 {
        state.write_msg_buff("Never mind.");
        return 0.0;
    } else {
        let tile = &state.map[&player_loc];                        
        let into_well = *tile == Tile::Well;
        if amt >= purse {
            if into_well {
                state.write_msg_buff("You hear faint tinkling splashes.");
            } else {
                state.write_msg_buff("You drop all your money.");
                let zorkmids = GoldPile::make(game_obj_db, purse, player_loc);
                game_obj_db.add(zorkmids);
            }                
            purse = 0;
        } else if amt > 1 {
            if into_well {
                state.write_msg_buff("You hear faint tinkling splashes.");
            } else {
                let s = format!("You drop {} gold pieces.", amt);
                state.write_msg_buff(&s);
                let zorkmids = GoldPile::make(game_obj_db, amt, player_loc);
                game_obj_db.add(zorkmids);
            }
            purse -= amt;
        } else {
            if into_well {
                state.write_msg_buff("You hear a faint splash.");
            } else {
                state.write_msg_buff("You drop a gold piece.");
                let zorkmids = GoldPile::make(game_obj_db, 1, player_loc);
                game_obj_db.add(zorkmids);                    
            }
            purse -= 1;
        }
    }

    let player = game_obj_db.player().unwrap();
    player.purse = purse;

    1.0
}

fn item_hits_ground(mut obj: GameObjects, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB) {
    obj.set_loc(loc);
    if let GameObjects::Item(item) = &mut obj {
        item.equiped = false;
    }
    game_obj_db.add(obj);
}

fn drop_stack(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), slot: char, count: u32) -> f32 {
    let player = game_obj_db.player().unwrap();
    let result = player.inv_remove_from_slot(slot, count);

    match result {
        Ok(pile) => {
            if !pile.is_empty() {
                for obj in pile {                        
                    let s = format!("You drop {}.", &obj.get_fullname().with_def_article());
                    state.write_msg_buff(&s);
                    item_hits_ground(obj, loc, game_obj_db);
                }
                return 1.0; // Really, dropping several items should take several turns...
            }
        },
        Err(msg) => state.write_msg_buff(&msg),
    }
    
    0.0
}

fn drop_item(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {    
    let sbi = state.curr_sidebar_info(game_obj_db);
    let player = game_obj_db.player().unwrap();
    let player_loc = player.get_loc();

    if player.purse == 0 && player.inventory.is_empty() {
        state.write_msg_buff("You are empty handed.");
        return 0.0;
    }
    
    let mut cost = 0.0;
    if let Some(ch) = gui.query_single_response("Drop what?", Some(&sbi)) {
        if ch == '$' {
            return drop_zorkmids(state, game_obj_db, gui);
        } else {
            let count = player.inv_count_in_slot(ch);
            if count == 0 {
                state.write_msg_buff("You do not have that item.");
            } else if count > 1 {
                match gui.query_natural_num("Drop how many?", Some(&sbi)) {
                    Some(v) => {
                        cost = drop_stack(state, game_obj_db, player_loc, ch, v);
                    },
                    None => state.write_msg_buff("Nevermind"),
                }
            } else {
                let result = player.inv_remove_from_slot(ch, 1);
                match result {
                    Ok(mut items) => {
                        let obj = items.remove(0);
                        let s = format!("You drop {}.", &obj.get_fullname().with_def_article());
                        state.write_msg_buff(&s);
                        item_hits_ground(obj, player_loc, game_obj_db);
                        cost = 1.0;
                    },
                    Err(msg) => state.write_msg_buff(&msg),
                }                    
            }
        }
    } else {
        state.write_msg_buff("Nevermind.");            
    }
    
    let player = game_obj_db.player().unwrap();
    player.calc_gear_effects();

    cost    
}

fn search_loc(state: &mut GameState, roll: u8, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB) {
    let things:Vec<usize> = game_obj_db.hidden_at_loc(loc);

    for obj_id in &things {
        if roll >= 15 {
            let t = game_obj_db.get_mut(*obj_id).unwrap();
            let s = format!("You find {}!", t.get_fullname().with_indef_article());
            state.write_msg_buff(&s);            
            t.reveal();
        }
    }
}

fn search(state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let player = game_obj_db.player().unwrap();
    let ploc = player.get_loc();
    
    let roll = player.ability_check(Ability::Apt);
    
    search_loc(state, roll, ploc, game_obj_db);
    for adj in util::ADJ.iter() {
        let loc = (ploc.0 + adj.0, ploc.1 + adj.1, ploc.2);
        search_loc(state, roll, loc, game_obj_db);
    }
}

// Not yet handling when there are no inventory slots yet
fn pick_up(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let things = game_obj_db.things_at_loc(player_loc);

    if things.is_empty() {
        state.write_msg_buff("There is nothing here.");
        return 0.0;
    } else if things.len() == 1 {
        let obj = game_obj_db.get(things[0]).unwrap();
        let zorkmids = if let GameObjects::GoldPile(_) = &obj {
            true
        } else {
            false
        };

        if zorkmids {
            let obj = game_obj_db.remove(things[0]);
            let amount = if let GameObjects::GoldPile(zorkmids) = &obj {
                zorkmids.amount
            } else {
                0
            };

            if amount == 1 {
                state.write_msg_buff(&"You pick up a single gold piece.");
            } else {
                let s = format!("You pick up {} gold pieces.", amount);
                state.write_msg_buff(&s);
            }
            
            let p = game_obj_db.player().unwrap();
            p.purse += amount;
        } else {
            let obj = game_obj_db.remove(things[0]);
            let s = format!("You pick up {}.", obj.get_fullname().with_def_article());
            state.write_msg_buff(&s);
            let p = game_obj_db.player().unwrap();
            p.add_to_inv(obj);
        }

        return 1.0;
    } else {
        let mut m = game_obj_db.get_pickup_menu(player_loc);
        let mut answer_key = HashMap::new();
        let mut menu = Vec::new();
        for (j, item) in m.iter().enumerate() {
            if item.0.contains("gold piece") {
                menu.push((item.0.to_string(), '$'));
                answer_key.insert('$', item.1);
                m.remove(j);
                break;
            }
        }
        for (j, item) in m.iter().enumerate() { // in 0..m.len() {
            let ch = (j as u8 + b'a') as char;
            menu.push((item.0.to_string(), ch));
            answer_key.insert(ch, item.1);
        }
        
        if let Some(answers) = gui.menu_picker("Pick up what: (* to get everything)".to_string(), &menu, false, true) {
            let picks: Vec<usize> = answers.iter().map(|a| answer_key[a]).collect();
            for id in picks {
                let obj = game_obj_db.get(id).unwrap();
                let (is_zorkmids, amount) = if let GameObjects::GoldPile(zorkmids) = &obj {
                    (true, zorkmids.amount)
                } else {
                    (false, 0)
                };
                
                if is_zorkmids {
                    if amount == 1 {
                        state.write_msg_buff("You pick up a single gold piece.");
                    } else {
                        let s = format!("You pick up {} gold pieces.", amount);
                        state.write_msg_buff(&s);
                    }
                    game_obj_db.remove(id);
                    game_obj_db.player().unwrap().purse += amount;                   
                } else {
                    let obj = game_obj_db.remove(id);
                    let s = format!("You pick up {}.", obj.get_fullname().with_def_article());
                    state.write_msg_buff(&s);
                    let p = game_obj_db.player().unwrap();
                    p.add_to_inv(obj);
                }
            }
            return 1.0;
        } else {
            state.write_msg_buff("Nevermind.");
        }
    }

    0.0
}

fn toggle_item(state: &mut GameState, player: &mut Player, slot: char) -> f32 {
    let obj = player.inv_item_in_slot(slot).unwrap();
    let obj_id = obj.obj_id();
    let (equipable, item_type, attributes) = if let GameObjects::Item(item) = &obj {
        (item.equipable(), item.item_type, item.attributes)
    } else {
        (false, ItemType::Note, 0)
    };

    if !equipable {
            state.write_msg_buff("You cannot wear or wield that!");
            return 0.0;
    }
    
    let mut swapping = false;
    if item_type == ItemType::Weapon {
        if attributes & items::IA_TWO_HANDED > 0 && player.readied_obj_ids_of_type(ItemType::Shield).len() > 0 {
            state.write_msg_buff("You cannot wield that while using a shield.");
            return 0.0;
        }

        let readied = player.readied_obj_ids_of_type(ItemType::Weapon);
        if !readied.is_empty() && readied[0] != obj_id {
            swapping = true;
            if let Some(GameObjects::Item(other)) = player.inv_obj_of_id(readied[0]) {
                other.equiped = false;
            }
        }        
    } else if item_type == ItemType::Armour {
        let readied = player.readied_obj_ids_of_type(ItemType::Armour);
        if !readied.is_empty() && readied[0] != obj_id {
            state.write_msg_buff("You're already wearing armour.");
            return 0.0;             
        }
    } else if item_type == ItemType::Shield {
        let readied = player.readied_obj_ids_of_type(ItemType::Shield);
        if !readied.is_empty() && readied[0] != obj_id {
            state.write_msg_buff("You're already wielding a shield.");
            return 0.0;
        }

        if let Some((weapon, _)) = player.readied_weapon() {
            if weapon.attributes & items::IA_TWO_HANDED > 0 {
                state.write_msg_buff("You cannot equip that along with a two-handed weapon!");
                return 0.0;
            }
        }
    }

    // // Alright, so at this point we can toggle the item in the slot.
    if let Some(GameObjects::Item(item)) = &mut player.inv_obj_of_id(obj_id) {
        let equiped = item.equiped;
        item.equiped = !equiped;

        let mut s = String::from("You ");
        if swapping {
            s.push_str("are now wielding ");
        } else if !equiped {
            s.push_str("equip ");
        } else {
            s.push_str("unequip ");
        }
        s.push_str(&item.get_fullname().with_def_article());
        s.push('.');
        state.write_msg_buff(&s);
    }
    
    player.calc_gear_effects();

    let readied = player.readied_obj_ids_of_type(ItemType::Weapon);
    if item_type == ItemType::Weapon && readied.is_empty() {
        state.write_msg_buff("You are now empty handed.");
    } 

    1.0
}

fn toggle_equipment(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {
    let sbi = state.curr_sidebar_info(game_obj_db);    
    let player = game_obj_db.player().unwrap();
    let slots = player.inv_slots_used();
    
    if slots.is_empty() {
        state.write_msg_buff("You are empty handed.");
        return 0.0;
    }

    let cost = if let Some(ch) = gui.query_single_response("Ready/unready what?", Some(&sbi)) {
        if !slots.contains(&ch) {
            state.write_msg_buff("You do not have that item!");
            0.0
        } else {
            toggle_item(state, player, ch)
        }
    } else {
        state.write_msg_buff("Nevermind.");
        0.0
    };

    cost
}

fn use_item(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {
    let sbi = state.curr_sidebar_info(game_obj_db);        
    let player = game_obj_db.player().unwrap();
    let slots = player.inv_slots_used();
    
    if slots.is_empty() {
        state.write_msg_buff("You are empty handed.");
        return 0.0;
    }
    
    if let Some(ch) = gui.query_single_response("Use what?", Some(&sbi)) {
        if !slots.contains(&ch) {
            state.write_msg_buff("You do not have that item!");
            return 0.0;
        }
        
        let obj = player.inv_item_in_slot(ch).unwrap();
        let obj_id = obj.obj_id();
        let (useable, item_type, consumable, effects) = if let GameObjects::Item(item) = &obj {
            (item.useable(), item.item_type, item.attributes & IA_CONSUMABLE > 0, item.effects)
        } else {
            (false, ItemType::Zorkmid, false, 0)
        };
        
        let (desc, text) = if let GameObjects::Item(item) = &obj {
            if let Some(text) = &item.text {
                (text.0.with_indef_article().capitalize(), text.1.clone())
            } else {
                ("".to_string(), "".to_string())
            }
        } else {
            ("".to_string(), "".to_string())
        };
        
        if useable {
            if item_type == ItemType::Light {
                let (item_id, active) = use_light(state, ch, game_obj_db);
                
                if active {
                    game_obj_db.listeners.insert((item_id, EventType::Update));
                    game_obj_db.listeners.insert((item_id, EventType::EndOfTurn));
                } else {
                    game_obj_db.listeners.remove(&(item_id, EventType::Update));
                    game_obj_db.listeners.remove(&(item_id, EventType::EndOfTurn));
                }
            }

            if effects > 0 {
                effects::apply_effects(state, 0, game_obj_db, effects)
            }

            if consumable {
                let player = game_obj_db.player().unwrap();
                player.inv_remove(obj_id);
            }

            return 1.0;
        } else if !text.is_empty() {
            gui.popup_msg(&desc, &text, Some(&sbi));
        } else {
            state.write_msg_buff("You don't know how to use that.");
        }       
    } else {
        state.write_msg_buff("Nevermind.");
    }

    0.0
}

fn use_light(state: &mut GameState, slot: char,game_obj_db: &mut GameObjectDB) -> (usize, bool) {
    let player = game_obj_db.player().unwrap();
    let next_slot = player.next_slot; // We might need to give the item a new inventory slot
    let was_in_stack = player.inv_count_in_slot(slot) > 1;

    let obj = player.inv_item_in_slot(slot).unwrap();
    let obj_id = obj.obj_id();
    let name = obj.get_fullname();

    let mut active = false;
    if let GameObjects::Item(item) = obj {
        let s = if item.active { 
            format!("You extinguish {}.", name.with_def_article())
        } else {
            format!("{} blazes brightly!", name.with_def_article().capitalize())
        };
        state.write_msg_buff(&s);
        item.active = !item.active;
        item.stackable = false;
        if was_in_stack {
            item.slot = next_slot;
        }
        active = item.active;

        if was_in_stack {
            player.inc_next_slot();
        }
    }
    
    (obj_id, active)    
}

fn get_move_tuple(mv: &str) -> (i32, i32) {
    if mv == "N" {
        (-1, 0)
    } else if mv == "S" {
        (1, 0)
    } else if mv == "W" {
        (0, -1)
    } else if mv == "E" {
        (0, 1)
    } else if mv == "NW" {
        (-1, -1)
    } else if mv == "NE" {
        (-1, 1)
    } else if mv == "SW" {
        (1, -1)
    } else {
        (1, 1)
    }
}

fn do_open(state: &mut GameState, loc: (i32, i32, i8)) {
    let tile = &state.map[&loc];
    match tile {
        Tile::Door(DoorState::Open) | Tile::Door(DoorState::Broken) => state.write_msg_buff("The door is already open!"),
        Tile::Door(DoorState::Closed) => {
            state.write_msg_buff("You open the door.");
            state.map.insert(loc, map::Tile::Door(DoorState::Open));
        },
        Tile::Door(DoorState::Locked) => state.write_msg_buff("That door is locked!"),
        _ => state.write_msg_buff("You cannot open that!"),
    }        
}

fn do_close(state: &mut GameState, loc: (i32, i32, i8)) {
    let tile = &state.map[&loc];
    match tile {
        Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) => state.write_msg_buff("The door is already closed!"),
        Tile::Door(DoorState::Open) => {
            state.write_msg_buff("You close the door.");
            state.map.insert(loc, map::Tile::Door(DoorState::Closed));
        },
        Tile::Door(DoorState::Broken) => state.write_msg_buff("That door is broken."),
        _ => state.write_msg_buff("You cannot open that!"),
    }        
}

fn take_stairs(state: &mut GameState, game_obj_db: &mut GameObjectDB, down: bool) -> f32 {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let tile = &state.map[&player_loc];

    if down {
        let cost = if *tile == map::Tile::Portal {
            state.write_msg_buff("You enter the beckoning portal.");
            game_obj_db.set_to_loc(0, (player_loc.0, player_loc.1, player_loc.2 + 1));
            1.0
        } else if *tile == map::Tile::StairsDown {
            state.write_msg_buff("You brave the stairs downward.");
            game_obj_db.set_to_loc(0, (player_loc.0, player_loc.1, player_loc.2 + 1));
            1.0
        } else {
            state.write_msg_buff("You cannot do that here.");
            0.0
        };

        if let Some(GameObjects::Player(p)) = game_obj_db.get_mut(0) {
            if player_loc.2 > p.max_depth as i8 {
                p.max_depth = player_loc.2 as u8 + 1;
            }
            
        }

        return cost;
    } else {
        if *tile == map::Tile::StairsUp {
            state.write_msg_buff("You climb the stairway.");
            game_obj_db.set_to_loc(0, (player_loc.0, player_loc.1, player_loc.2 - 1));
            
            if player_loc.2 == 0 {
                state.write_msg_buff("Fresh air!");
            }

            return 1.0;
        } else {
            state.write_msg_buff("You cannot do that here.");
        }
    }

    0.0
}

fn check_closed_gate(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8)) {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let mut rng = rand::thread_rng();
    if player_loc == loc {
        let mut options: Vec<usize> = (0..util::ADJ.len()).collect();            
        options.shuffle(&mut rng);
        while !options.is_empty() {
            let id = options.pop().unwrap();
            let landing_spot = (loc.0 + util::ADJ[id].0, loc.1 + util::ADJ[id].1, loc.2);
            if !state.map[&landing_spot].passable() {
                continue;
            }
            if !game_obj_db.location_occupied(&landing_spot) {
                state.write_msg_buff("You are shoved out of the way by the falling gate!");
                game_obj_db.set_to_loc(0, landing_spot);
                return;
            }
        }

        // If we get here there are no available landing spots. What to do?
        // Just crush the player to death??           
    } else if let Some(obj_id) = game_obj_db.npc_at(&loc) {
        // This is untested because I don't have NPCs aside from villagers in the game...
        let mut options: Vec<usize> = (0..util::ADJ.len()).collect();            
        options.shuffle(&mut rng);
        while !options.is_empty() {
            let id = options.pop().unwrap();                
            let landing_spot = (loc.0 + util::ADJ[id].0, loc.1 + util::ADJ[id].1, loc.2);
            if !state.map[&landing_spot].passable() {
                continue;
            }
            if landing_spot != player_loc && !game_obj_db.location_occupied(&landing_spot) {
                let npc = game_obj_db.get(obj_id).unwrap();
                let npc_name = npc.get_fullname();
                let start_loc = npc.get_loc();
                let npc_id = npc.obj_id();
                
                game_obj_db.set_to_loc(npc_id, landing_spot);
                
                let s = format!("{} is shoved out of the way by the falling gate!", npc_name.with_def_article());
                state.write_msg_buff(&s);

                game_obj_db.remove_from_loc(npc_id, start_loc);                    
                game_obj_db.stepped_on_event(state, landing_spot);

                return;
            }
        }
    }
}

fn firepit_msg(num: u8) -> &'static str {
    if num == 0 {
        "An old fire pit -- some previous adventurer?"
    } else if num == 1 {
        "A long dead campfire."
    } else if num == 2 {
        "Some of the bones in the fire look human-shaped..."
    } else if num == 3 {
        "The ashes are cold."
    } else {
        "You see the remnants of a cooked rat."
    }
}

fn maybe_fight(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), gui: &mut GameUI) -> f32 {
    if let Some(npc_id) = game_obj_db.npc_at(&loc) {
        let npc = game_obj_db.get_mut(npc_id).unwrap();
        let (npc_name, attitude) = if let GameObjects::NPC(other) = npc {
            (other.npc_name(false), other.attitude)
        } else {
            ("".to_string(), Attitude::Indifferent)
        };
        
        match attitude {
            Attitude::Hostile => {
                battle::player_attacks(state, npc_id, game_obj_db);
                return 1.0;
            },
            Attitude::Indifferent | Attitude::Stranger => {
                let sbi = state.curr_sidebar_info(game_obj_db);
                let s = format!("Really attack {}? (y/n)", npc_name);
                if let 'y' = gui.query_yes_no(&s, Some(&sbi)) {
                    let npc = game_obj_db.get_mut(npc_id).unwrap();
                    if let GameObjects::NPC(foe) = npc {
                        foe.attitude = Attitude::Hostile;
                        foe.active = true;
                    }                    
                    battle::player_attacks(state, npc_id, game_obj_db);
                    return 1.0;
                }                    
            },
            _ => {
                let s = format!("{} is in your way!", npc_name.capitalize());
                state.write_msg_buff(&s);
                return 1.0;
            }
        }
    }

    0.0
}

fn random_open_sq(state: &mut GameState, game_obj_db: &GameObjectDB, level: i8) -> (i32, i32, i8) {
    let mut rng = rand::thread_rng();

    let all_sqs_on_level: Vec<(i32, i32, i8)> = state.map.keys()
        .filter(|k| k.2 == level)
        .map(|k| *k).collect();

    loop {
        let i = rng.gen_range(0, all_sqs_on_level.len());
        let loc = all_sqs_on_level[i];
        if state.map[&loc].passable_dry_land() && !game_obj_db.blocking_obj_at(&loc) {
            return loc;
        }
    }
}

// Stuff that happens after someone steps on a square. I could probably move a bunch of the code here for
// stepping on lava, etc. It's a bit awkward right now because Player and NPC are separate types and I can't
// just pass a reference in, but if I eventually need to, I can sort out who exactly stepped on the square via
// the obj_id (0 is always the player)
fn land_on_location(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), _obj_id: usize) {
    game_obj_db.stepped_on_event(state, loc);

    // for special in game_objs.special_sqs_at_loc(&loc) {
    //     if special.special_sq.as_ref().unwrap().get_tile() == Tile::TeleportTrap {
            
    //     }
    // }
}

fn do_move(state: &mut GameState, game_obj_db: &mut GameObjectDB, dir: &str, gui: &mut GameUI) -> f32 {
    let mv = get_move_tuple(dir);

    let start_loc = game_obj_db.get(0).unwrap().get_loc();
    let start_tile = state.map[&start_loc];
    let next_loc = (start_loc.0 + mv.0, start_loc.1 + mv.1, start_loc.2);
    let tile = state.map[&next_loc].clone();
    
    // Ugh, this function is turning into a hot mess. I need to break it up into the attempt to 
    // move to or from a square and the effects of landing on a square. With difficult terrain
    // for instance, if you are stuck on a square you should suffer its effects again. Ie., if
    // it is difficult terrain to move out of lava

    if game_obj_db.blocking_obj_at(&next_loc) {
        return maybe_fight(state, game_obj_db, next_loc, gui);            
    } else if tile.passable() {
        // Rubble is difficult terrain and requires a dex check to mvoe off of.
        // (If you designate more terrain types as difficult terrain, probably make a function)
        if start_tile == Tile::Rubble {
            if let Some(GameObjects::Player(p)) = game_obj_db.get(0) {
                if p.ability_check(Ability::Dex) <= 12 {
                    state.write_msg_buff("You stumble and trip over the rubble.");
                    return 1.0;
                }
            }
        }

        match tile {
            Tile::Water => state.write_msg_buff("You splash in the shallow water."),
            Tile::Rubble => {
                if start_tile != Tile::Rubble {
                    state.write_msg_buff("The ground is cracked and rubble-strewn here.")
                }
            },
            Tile::DeepWater => {
                if start_tile != Tile::DeepWater {
                    state.write_msg_buff("You wade into the flow.");				
                }
            },
            Tile::Well => state.write_msg_buff("A well."),
            Tile::Lava => state.write_msg_buff("MOLTEN LAVA!"),
            Tile::FirePit => {
                state.write_msg_buff("You step in the fire!");
            },
            Tile::OldFirePit(n) => state.write_msg_buff(firepit_msg(n)),
            Tile::Portal => state.write_msg_buff("Where could this lead..."),
            Tile::Shrine(stype) => {
                match stype {
                    ShrineType::Woden => state.write_msg_buff("A shrine to Woden."),
                    ShrineType::Crawler => state.write_msg_buff("The misshapen altar makes your skin crawl"),
                }
            },
            _ => {
                if start_tile == map::Tile::DeepWater { 
                    state.write_msg_buff("Whew, you stumble ashore.");
                } else if state.aura_sqs.contains(&next_loc) && !state.aura_sqs.contains(&start_loc) {
                    state.write_msg_buff("You feel a sense of peace.");
                }
            },            
        }

        let items = game_obj_db.descs_at_loc(&next_loc);
        let item_count = items.len();                        
        if item_count == 1 {
            let s = format!("You see {} here.", items[0]);
            state.write_msg_buff(&s);
        } else if item_count == 2 {
            let s = format!("You see {} and {} here.", items[0], items[1]);
            state.write_msg_buff(&s);
        } else if item_count > 2 {
            state.write_msg_buff("There are several items here.");
        }
        
        game_obj_db.set_to_loc(0, next_loc);
        
        // This whole next section of checking for special floor effects is gross and ugly
        // but I don't know what the final form will look like after I have more kinds of 
        // effects so I'm going to leave it gross until it's more fixed.
        game_obj_db.stepped_on_event(state, next_loc);

        let mut teleport: bool = false;
        for special in game_obj_db.special_sqs_at_loc(&next_loc) {
            if special.get_tile() == Tile::TeleportTrap {
                teleport = true;
            }
        }
        if teleport {
            let sq = random_open_sq(state, game_obj_db, start_loc.2);
            game_obj_db.set_to_loc(0, sq);                
        }

        return 1.0;
    } else if tile == Tile::Door(DoorState::Closed) {
        // Bump to open doors. I might make this an option later
        do_open(state, next_loc);
        return 1.0;
    } else if tile == Tile::Gate(DoorState::Closed) || tile == Tile::Gate(DoorState::Locked) {
        state.write_msg_buff("A portcullis bars your way.");    
    } else  {
        state.write_msg_buff("You cannot go that way.");
    }

    0.0
}

fn chat_with(state: &mut GameState, gui: &mut GameUI, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB, dialogue: &DialogueLibrary) -> f32 {
    let sbi = state.curr_sidebar_info(game_obj_db);
    if let Some(obj_id) = game_obj_db.npc_at(&loc) {
        let npc = game_obj_db.get_mut(obj_id).unwrap();

        let venue = if let GameObjects::NPC(npc) = npc {
            npc.home.as_ref()
        } else {
            None
        };
        match venue {
            Some(Venue::Tavern) => { 
                shops::talk_to_innkeeper(state, obj_id, game_obj_db, dialogue, gui);
            },
            Some(Venue::Market) => {
                shops::talk_to_grocer(state, obj_id, game_obj_db, dialogue, gui);
            },
            Some(Venue::Smithy) => {
                shops::talk_to_smith(state, obj_id, game_obj_db, dialogue, gui);
            },
            _ => {
                let mut ei = HashMap::new();
                if let GameObjects::NPC(npc) = npc {
                    let line = npc.talk_to(state, dialogue, &mut ei);
                    state.add_to_msg_history(&line);
                    gui.popup_msg(&npc.npc_name(true).capitalize(), &line, Some(&sbi));
                }
            },
        }           
    } else {
        if let Tile::Door(_) = state.map[&loc] {
            state.write_msg_buff("The door is ignoring you.");
        } else {
            state.write_msg_buff("Oh no, talking to yourself?");
        } 
    }

    1.0
}

fn show_character_sheet(gui: &mut GameUI, player: &Player) {
    let s = format!("{}, a {} level {}", player.get_fullname(), util::num_to_nth(player.level), player.role.desc());
    let mut lines: Vec<&str> = vec![&s];
    lines.push("");
    let s = format!("Strength: {}", player.str);
    lines.push(&s);
    let s = format!("Dexterity: {}", player.dex);
    lines.push(&s);
    let s = format!("Constitution: {}", player.con);
    lines.push(&s);
    let s = format!("Charisma: {}", player.chr);
    lines.push(&s);
    let s = format!("Aptitude: {}", player.apt);
    lines.push(&s);
    lines.push("");
    let s = format!("AC: {}    Hit Points: {}({})", player.ac, player.curr_hp, player.max_hp);
    lines.push(&s);
    let s = format!("XP: {}", player.xp);
    lines.push(&s);
    lines.push("");

    let dungeon_depth = if player.max_depth == 0 {
        String::from("You have not yet ventured into the dungeon.")
    } else {
        format!("You have been as far as the {} level of the dungeon.", util::num_to_nth(player.max_depth))
    };
    lines.push(&dungeon_depth);

    gui.write_long_msg(&lines, true);
}

fn show_inventory(gui: &mut GameUI, state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let p = game_obj_db.player().unwrap();
    let menu = p.inv_menu();
    let purse = p.purse;

    let money = if purse == 1 {
        String::from("$) a single zorkmid to your name")
    } else {
        let s = format!("$) {} gold pieces", purse);
        s
    };

    if menu.is_empty() && purse == 0 {
        state.write_msg_buff("You are empty-handed.");
    } else {
        let mut m: Vec<&str> = menu.iter().map(AsRef::as_ref).collect();        
        m.insert(0, "You are carrying:");
        if purse > 0 {
            m.insert(1, &money);
        }
        //gui.write_long_msg(&m, true);
        gui.show_in_side_pane(&m);
    }
}

fn dump_level(state: &GameState, level: i8) {
    let dungeon_sqs:  Vec<(i32, i32, i8)> = state.map.keys()
                                                    .filter(|k| k.2 == level)
                                                    .copied()
                                                    .collect();
    let min_row = dungeon_sqs.iter().map(|sq| sq.0).min().unwrap();
    let min_col = dungeon_sqs.iter().map(|sq| sq.1).min().unwrap();
    let max_col = dungeon_sqs.iter().map(|sq| sq.1).max().unwrap();
    let width = max_col - min_col + 1;
    let mut chars = vec![' '; dungeon_sqs.len()];
    for sq in dungeon_sqs {
        let row = sq.0 - min_row;
        let col = sq.1 - min_col;
        let ch = match state.map[&sq] {
                Tile::Wall => '#',
                Tile::StoneFloor => '.',
                Tile::Door(_) => '+',
                Tile::Shrine(_) => '_',
                Tile::Trigger => '^',
                Tile::OldFirePit(_) => '#',
                Tile::StairsDown => '>',
                Tile::StairsUp => '<',
                Tile::Gate(_) => '/',
                Tile::Rubble => ':',
                Tile::UndergroundRiver => '~',
                _ => ' ',
            };
        
        chars[(row * width + col) as usize] = ch;
    }

    let mut c = 0;
    let mut s = String::from("");
    while c < chars.len() {            
        s.push(chars[c]);
        c += 1;

        if c % width as usize == 0 {
            println!("{}", s);
            s = String::from("");
        }
    }        
}

fn wiz_command(state: &mut GameState, gui: &mut GameUI, game_obj_db: &mut GameObjectDB, mf: &MonsterFactory)  {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let sbi = state.curr_sidebar_info(game_obj_db);
    if let Some(result) = gui.query_user(":", 20, Some(&sbi)) {
        let pieces: Vec<&str> = result.trim().split('=').collect();

        if result == "loc" {
            println!("{:?}", player_loc);
        } else if result == "!heal" {
            let loc = (player_loc.0, player_loc.1, player_loc.2);
            let mut poh = items::Item::get_item(game_obj_db,"potion of healing").unwrap();
            poh.set_loc(loc);
            game_obj_db.add(poh);
        } else if result == "goblin" {
            let loc = (player_loc.0, player_loc.1 - 1, player_loc.2);
            mf.add_monster("goblin", loc, game_obj_db);
        } else if result == "dump level" {
            if player_loc.2 == 0 {
                state.write_msg_buff("Uhh the wilderness is too big to dump.");
            } else {
                dump_level(state, player_loc.2);
            }
        } else if pieces.len() != 2 {
            state.write_msg_buff("Invalid wizard command");
        } else if pieces[0] == "turn" {
            let num = pieces[1].parse::<u32>();
            match num {
                Ok(v) => state.turn = v,
                Err(_) => state.write_msg_buff("Invalid wizard command"),
            }
        } else {
            state.write_msg_buff("Invalid wizard command");
        }
    }
}

fn confirm_quit(state: &GameState, gui: &mut GameUI, game_obj_db: &mut GameObjectDB) -> Result<(), ExitReason> {
    let sbi = state.curr_sidebar_info(game_obj_db);
    if let 'y' = gui.query_yes_no("Do you really want to Quit? (y/n)", Some(&sbi)) {
        Err(ExitReason::Quit)
    } else {
        Ok(())
    }
}

fn pick_player_start_loc(state: &GameState) -> (i32, i32, i8) {
    let x = thread_rng().gen_range(0, 4);
    let b = state.world_info.town_boundary;

    for fact in &state.world_info.facts {
        if fact.detail == "dungeon location" {
            return fact.location;
        }
    }
    
    if x == 0 {
        (b.0 - 5, thread_rng().gen_range(b.1, b.3), 0)
    } else if x == 1 {
        (b.2 + 1, thread_rng().gen_range(b.1, b.3), 0)
    } else if x == 2 {
        (thread_rng().gen_range(b.0, b.2), b.1 - 5, 0)
    } else {
        (thread_rng().gen_range(b.0, b.2), b.3 + 5, 0)
    }
}

// The fov calculator returns a vector of co-ordinates and whether or not that square is currently visible.
// From that, we assemble the vector of tiles to send to the GameUI to be drawn. If an NPC is in a visible square,
// they are on top, otherwise show the tile. If the tile isn't visible but the player has seen it before, show the 
// tile as unlit, otherwise leave it as a blank square.
fn fov_to_tiles(state: &mut GameState, game_obj_db: &GameObjectDB, visible: &[((i32, i32, i8), bool)], player_loc: (i32, i32, i8)) -> Vec<(map::Tile, bool)> {
    let mut v_matrix = vec![(map::Tile::Blank, false); visible.len()];
    for j in 0..visible.len() {
        let vis = visible[j];
        if vis.0 == player_loc {
            v_matrix[j] = (map::Tile::Player(WHITE), true);
        } else if visible[j].1 {     
            let tile = if let Some(t) = game_obj_db.tile_at(&vis.0) {
                if t.1 {
                    state.tile_memory.insert(vis.0, t.0);
                }
                t.0
            } else {
                state.tile_memory.insert(vis.0, state.map[&vis.0]);
                
                // I wanted to make tochlight squares be coloured different so this is a slight
                // kludge. Although perhaps later I might use it to differentiate between a player
                // walking through the dungeon with a light vs relying on darkvision, etc
                if state.aura_sqs.contains(&vis.0) && state.map[&vis.0] == Tile::StoneFloor {
                    Tile::ColourFloor(display::LIGHT_BLUE)
                } else if state.lit_sqs.contains(&vis.0) {
                    match state.map[&vis.0] {
                        Tile::StoneFloor => Tile::ColourFloor(display::YELLOW),
                        Tile::Trigger => Tile::ColourFloor(display::YELLOW_ORANGE),
                        _ => state.map[&vis.0],
                    }
                } else {
                    state.map[&vis.0]
                }
            };
            
            v_matrix[j] = (tile, true);
        } else if state.tile_memory.contains_key(&vis.0) {
            v_matrix[j] = (state.tile_memory[&vis.0], false);            
        }
    }

    v_matrix
}

fn kill_screen(state: &mut GameState, gui: &mut GameUI, game_obj_db: &mut GameObjectDB, msg: &str) {
    if msg.is_empty() {
        state.write_msg_buff("Oh no! You have died!");
    } else {
        let s = format!("Oh no! You have been killed by {}!", msg);
        state.write_msg_buff(&s);
    }
    
    let s = format!("Farewell, {}.", game_obj_db.get(0).unwrap().get_fullname());
    state.write_msg_buff(&s);
    let sbi = state.curr_sidebar_info(game_obj_db);
    gui.write_screen(&state.msg_buff, Some(&sbi));
    gui.pause_for_more();
}

fn check_player_statuses(state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let p = game_obj_db.player().unwrap();
    let mut j = 0;
    while j < p.statuses.len() {
        match p.statuses[j] {
            Status::PassUntil(time) => {
                if time <= state.turn {
                    p.statuses.remove(j);
                    continue;
                }
            },
            Status::RestAtInn(time) => {
                if time <= state.turn {
                    p.statuses.remove(j);
                    state.write_msg_buff("You awake feeling refreshed!");
                    continue;
                }
            },
        }

        j += 1;
    }
}

// Herein lies the main game loop
fn run(gui: &mut GameUI, state: &mut GameState, game_obj_db: &mut GameObjectDB, dialogue: &DialogueLibrary, monster_fac: &MonsterFactory) -> Result<(), ExitReason> {    
    update_view(state, game_obj_db, gui);
    state.msg_buff.clear();

    loop {
        let mut curr_energy = 0.0;
        if let Some(GameObjects::Player(player)) = game_obj_db.get(0) {
            curr_energy = player.energy;
        }
        let mut skip_turn = false;
        while curr_energy >= 1.0 {
            gui.clear_msg_buff();

            if let Some(GameObjects::Player(p)) = game_obj_db.get(0) {
                for status in &p.statuses {
                    match status {
                        Status::PassUntil(_) => skip_turn = true,
                        Status::RestAtInn(_) => skip_turn = true,
                    }                
                }
            }

            let cmd = if skip_turn {
                Cmd::Pass
            } else  {
                gui.get_command(&state, game_obj_db)
            };

            let mut energy_cost = 0.0;
            match cmd {
                Cmd::Chat(loc) => energy_cost = chat_with(state, gui, loc, game_obj_db, dialogue),
                Cmd::Close(loc) => {
                    do_close(state, loc);
                    energy_cost = 1.0;
                },
                Cmd::Down => energy_cost = take_stairs(state, game_obj_db, true),
                Cmd::DropItem => energy_cost = drop_item(state, game_obj_db, gui),  
                Cmd::Move(dir) => energy_cost = do_move(state, game_obj_db, &dir, gui),
                Cmd::MsgHistory => show_message_history(state, gui),
                Cmd::Open(loc) => { 
                    do_open(state, loc);
                    energy_cost = 1.0;
                },
                Cmd::Pass => {
                    let p = game_obj_db.player().unwrap();
                    energy_cost = p.energy;
                },
                Cmd::PickUp => energy_cost = pick_up(state, game_obj_db, gui),
                Cmd::Save => save_and_exit(state, game_obj_db, gui)?,
                Cmd::Search => {
                    search(state, game_obj_db);
                    energy_cost = 1.0;
                },
                Cmd::ShowCharacterSheet => {
                    if let Some(GameObjects::Player(p)) = game_obj_db.get(0) {
                        show_character_sheet(gui, p);
                    }
                },
                Cmd::ShowInventory => show_inventory(gui, state, game_obj_db),
                Cmd::ToggleEquipment => energy_cost = toggle_equipment(state, game_obj_db, gui),
                Cmd::Use => energy_cost = use_item(state, game_obj_db, gui),
                Cmd::Quit => confirm_quit(state, gui, game_obj_db)?,
                Cmd::Up => energy_cost = take_stairs(state, game_obj_db, false),
                Cmd::WizardCommand => wiz_command(state, gui, game_obj_db, monster_fac),
                _ => continue,
            }
            
            let p = game_obj_db.player().unwrap();
            p.energy -= energy_cost;
            curr_energy = p.energy;
            if curr_energy >= 1.0 {
                // We need to do this here in case a player has enough energy to take multiple actions
                // and kills a monster on their first aciton.
                // I should queue an event "Monster killed" and then check the event queue. That way I don't
                // have to loop over the entire structure of GameObjects looking to see if there are any
                // dead ones.
                game_obj_db.check_for_dead_npcs();
                game_obj_db.update_listeners(state, EventType::Update);
                if !skip_turn || !state.msg_buff.is_empty() {
                    update_view(state, game_obj_db, gui);
                }
            }
        }

        check_player_statuses(state, game_obj_db);

        game_obj_db.do_npc_turns(state);
        game_obj_db.update_listeners(state, EventType::Update);
        game_obj_db.update_listeners(state, EventType::EndOfTurn);
        
        // Are there any accumulated events we need to deal with?
        while !state.queued_events.is_empty() {
            match state.queued_events.pop_front().unwrap() {
                (EventType::GateClosed, loc, _, _) => {
                    check_closed_gate(state, game_obj_db, loc);
                },
                (EventType::PlayerKilled, _, _, Some(msg)) => {
                    kill_screen(state, gui, game_obj_db, &msg);
                    return Err(ExitReason::Death(String::from("Player killed")));
                },
                (EventType::LevelUp, _, _, _) => {
                    let p = game_obj_db.player().unwrap();
                    p.level_up(state);
                }
                _ => { },
            }                
        }

        let p = game_obj_db.player().unwrap();
        p.energy += p.energy_restore;
        if state.turn % 25 == 0 {
             p.recover();
        }

        state.turn += 1;

        if !skip_turn || !state.msg_buff.is_empty() {
            update_view(state, game_obj_db, gui);
        }

        // if skip_turn {
        //     // Just a little pause if skipping turns so the CPU doesn't go crazy
        //     ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
        // }
    }
}

fn update_view(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) {
    let mut player_loc = (0, 0, 0);
    let mut player_vr = 0;
    if let Some(GameObjects::Player(player)) = game_obj_db.get_mut(0) {
        player_loc = player.get_loc();
        player.calc_vision_radius(state, player_loc);
        player_vr = player.vision_radius;
    }
        
    //let _fov_start = Instant::now();
    let visible = fov::visible_sqs(state, player_loc, player_vr, false);
    gui.v_matrix = fov_to_tiles(state, game_obj_db, &visible, player_loc);        
    //let _fov_duration = _fov_start.elapsed();
    //println!("Player fov: {:?}", fov_duration);
    
    //let write_screen_start = Instant::now();
    let sbi = state.curr_sidebar_info(game_obj_db);

    gui.write_screen(&state.msg_buff, Some(&sbi));
    state.msg_buff.clear();
    //let write_screen_duration = write_screen_start.elapsed();
    //println!("Time for write_screen(): {:?}", write_screen_duration); 
}

fn main() {
    let ttf_context = sdl2::ttf::init()
        .expect("Error creating ttf context on start-up!");
    let font_path: &Path = Path::new("DejaVuSansMono.ttf");
    let font = ttf_context.load_font(font_path, 24)
        .expect("Error loading game font!");
    let sm_font = ttf_context.load_font(font_path, 18)
        .expect("Error loading small game font!");
    let mut gui = GameUI::init(&font, &sm_font)
        .expect("Error initializing GameUI object.");

    title_screen(&mut gui);

    let mf = MonsterFactory::init();
    let dialogue_library = dialogue::read_dialogue_lib();
    let player_name = who_are_you(&mut gui);
    
    let mut game_obj_db: GameObjectDB;
    let mut state: GameState;
    if existing_save_file(&player_name) {
        if let Some(saved_objs) = fetch_saved_data(&player_name) {
            state = saved_objs.0;
            game_obj_db = saved_objs.1;
            
            let msg = format!("Welcome back, {}!", player_name);
            state.write_msg_buff(&msg);
        } else {
            // need to dump some sort of message for corrupted game file
            return;
        }
    } else {
        game_obj_db = GameObjectDB::new();

        let wg_start = Instant::now();
        let w = world::generate_world(&mut game_obj_db, &mf, &player_name);        
        state = GameState::init(w.0, w.1);    
        let wg_dur = wg_start.elapsed();
        println!("World gen time: {:?}", wg_dur);

        start_new_game(&state, &mut game_obj_db, &mut gui, player_name);
        
        state.write_msg_buff("Welcome, adventurer!");
    }
    
    match run(&mut gui, &mut state, &mut game_obj_db, &dialogue_library, &mf) {
        Ok(_) => println!("Game over I guess? Probably the player won?!"),
        //Err(ExitReason::Save) => save_msg(&mut state, &mut gui),
        //Err(ExitReason::Quit) => quit_msg(&mut state, &mut gui),
        //Err(ExitReason::Win) => victory_msg(&mut state, &mut gui),
        //Err(ExitReason::Death(src)) => death(&mut state, src, &mut gui),
        Err(_) => println!("okay bye"),
    }
}
