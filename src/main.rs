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

mod battle;
mod dialogue;
mod display;
mod dungeon;
mod effects;
mod game_obj;
mod fov;
mod items;
mod map;
mod npc;
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
use std::time::Duration;
use std::path::Path;

use std::time::Instant;

use rand::{Rng, prelude::SliceRandom, thread_rng};
use serde::{Serialize, Deserialize};

use battle::DamageType;
use dialogue::DialogueLibrary;
use display::{Colour, GameUI, SidebarInfo, WHITE};
use effects::{HasStatuses, Status};
use game_obj::{Ability, GameObject, GameObjectDB, GameObjects, Person};
use items::{GoldPile, IA_CONSUMABLE, IA_IMMOBILE, Item, ItemType};
use map::{DoorState, ShrineType, Tile};
use npc::{Attitude, MA_WEBSLINGER, MonsterFactory, Venue};
use player::{Player};
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
    DeathOf(usize),
}

pub enum Cmd { 
    Bash((i32, i32, i8)),
    Chat((i32, i32, i8)),    
    Close((i32, i32, i8)),
    Down,
    DropItem,
    Help,    
    Move(String),
    MsgHistory,
    Open((i32, i32, i8)),
    Pass,
    PickUp,
    Quit,
    Save,
    Search,
    ShowCharacterSheet,
    ShowInventory,
    ToggleEquipment,
    Up,
    Use,
    WizardCommand,
}

#[derive(Debug)]
pub struct ConfigOptions {
    font_size: u16,
    sm_font_size: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    obj_id: usize,
    loc: (i32, i32, i8),
    text: String,
    alt_text: String,
}

impl Message {
    pub fn new(obj_id: usize, loc: (i32, i32, i8), text: &str, alt_text: &str) -> Message {
        Message { obj_id, loc, text: String::from(text), alt_text: String::from(alt_text) }
    }

    // for messages that are direct to the player and don't rely on needing to see the source.
    pub fn info(text: &str) -> Message {
        Message { obj_id: 0, loc: (-1, -1, -1), text: String::from(text), alt_text: String::from("") }
    }
}

#[derive(Serialize, Deserialize)]
pub struct GameState {
    msg_queue: VecDeque<Message>,
    map: Map,
    turn: u32,
    world_info: WorldInfo,
    tile_memory: HashMap<(i32, i32, i8), Tile>,
    lit_sqs: HashMap<(i32, i32, i8), Colour>, // by light sources independent of player
    aura_sqs: HashSet<(i32, i32, i8)>, // areas of special effects
    queued_events: VecDeque<(EventType, (i32, i32, i8), usize, Option<String>)>, // events queue during a turn that should be resolved at the end of turn
    animation_pause: bool,
    curr_visible: HashSet<(i32, i32, i8)>,
}

impl GameState {
    pub fn init(map: Map, world_info: WorldInfo) -> GameState {
        GameState {
            msg_queue: VecDeque::new(),
            map,
            turn: 0,
            world_info: world_info,
            tile_memory: HashMap::new(),
            lit_sqs: HashMap::new(),
            aura_sqs: HashSet::new(),
            queued_events: VecDeque::new(),
            animation_pause: false,
            curr_visible: HashSet::new(),
        }
    }

    pub fn curr_sidebar_info(&self, game_obj_db: &mut GameObjectDB) -> SidebarInfo {
        let player = game_obj_db.player().unwrap();
        let loc = player.get_loc();
        let weapon_name = match player.readied_weapon() {
            Some(res) => res.1.capitalize(),
            None => "Empty handed".to_string(),
        };
        
        let mut poisoned = false;
        let mut confused = false;
        let mut paralyzed = false;
        for s in player.statuses.iter() {
            match s {
                Status::WeakVenom(_) => { poisoned = true; },
                Status::ConfusedUntil(_) => { confused = true; },
                Status::Paralyzed(_) => { paralyzed = true; },
                _ => { },
            }
        }

        SidebarInfo::new(player.get_fullname(), player.curr_hp, player.max_hp, self.turn, player.ac,
            player.purse, weapon_name, loc.2 as u8, poisoned, confused, paralyzed)
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
    let sbi = state.curr_sidebar_info(game_obj_db);

    if purse == 0 {
        state.msg_queue.push_back(Message::info("You have no money!"));        
        return 0.0;
    }

    if let Some(amt) = gui.query_natural_num("How much?", Some(&sbi)) {
        let tile = &state.map[&player_loc];                        
        let into_well = *tile == Tile::Well;

        if amt == 0 {
            state.msg_queue.push_back(Message::info("Never mind."));            
        } else if amt >= purse {
            if into_well {
                state.msg_queue.push_back(Message::info("You hear faint tinkling splashes."));                
            } else {
                state.msg_queue.push_back(Message::info("You drop all of your money."));
                let zorkmids = GoldPile::make(game_obj_db, purse, player_loc);
                game_obj_db.add(zorkmids);
            }                
            purse = 0;
        } else if amt > 1 {
            if into_well {
                state.msg_queue.push_back(Message::info("You hear faint tinkling splashes."));
            } else {
                let s = format!("You drop {} gold pieces.", amt);
                state.msg_queue.push_back(Message::info(&s));                
                let zorkmids = GoldPile::make(game_obj_db, amt, player_loc);
                game_obj_db.add(zorkmids);
            }
            purse -= amt;
        } else {
            if into_well {
                state.msg_queue.push_back(Message::info("You hear a faint splash."));
            } else {
                state.msg_queue.push_back(Message::info("You drop a gold piece."));
                let zorkmids = GoldPile::make(game_obj_db, 1, player_loc);
                game_obj_db.add(zorkmids);                    
            }
            purse -= 1;
        }
    } else {
        state.msg_queue.push_back(Message::info("Never mind."));
        return 0.0;
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
                    state.msg_queue.push_back(Message::info(&s));                    
                    item_hits_ground(obj, loc, game_obj_db);
                }
                return 1.0; // Really, dropping several items should take several turns...
            }
        },
        Err(msg) => state.msg_queue.push_back(Message::info(&msg)),
    }
    
    0.0
}

fn drop_item(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {    
    let sbi = state.curr_sidebar_info(game_obj_db);
    let player = game_obj_db.player().unwrap();
    let player_loc = player.get_loc();

    if player.purse == 0 && player.inventory.is_empty() {
        state.msg_queue.push_back(Message::info("You are empty handed."));
        return 0.0;
    }
    
    let mut cost = 0.0;
    let mut menu =  player.inv_menu(0);
    if player.purse > 0 {
        let mut s = format!("$) {} gold piece", player.purse);
        if player.purse > 1 {
            s.push('s');
        }
        menu.insert(0, (s, true));
    }
    if let Some(ch) = gui.show_in_side_pane("Drop which?", &menu) {
        if ch == '$' {
            return drop_zorkmids(state, game_obj_db, gui);
        } else {
            let count = player.inv_count_in_slot(ch);
            if count == 0 {
                state.msg_queue.push_back(Message::info("You do not have that item."));
            } else if count > 1 {
                match gui.query_natural_num("Drop how many?", Some(&sbi)) {
                    Some(v) => {
                        cost = drop_stack(state, game_obj_db, player_loc, ch, v);
                    },
                    None => state.msg_queue.push_back(Message::info("Never mind.")),
                }
            } else {
                let result = player.inv_remove_from_slot(ch, 1);
                match result {
                    Ok(mut items) => {
                        let obj = items.remove(0);
                        let s = format!("You drop {}.", &obj.get_fullname().with_def_article());
                        state.msg_queue.push_back(Message::info(&s));                        
                        item_hits_ground(obj, player_loc, game_obj_db);
                        cost = 1.0;
                    },
                    Err(msg) => state.msg_queue.push_back(Message::info(&msg)),
                }                    
            }
        }
    } else {
        state.msg_queue.push_back(Message::info("Never mind."));
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
            state.msg_queue.push_back(Message::info(&s));
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
    let things = game_obj_db.items_to_pick_up(player_loc);
    
    if things.is_empty() {
        state.msg_queue.push_back(Message::info("There is nothing to pick up here."));        
        return 0.0;
    } else if things.len() == 1 {
        let obj = game_obj_db.get(things[0]).unwrap();
        let zorkmids = if let GameObjects::GoldPile(_) = &obj {
            true
        } else {
            false
        };

        if let GameObjects::Item(item) = obj {
            if item.attributes & IA_IMMOBILE > 0 {
                state.msg_queue.push_back(Message::info("You cannot pick that up!"));
                return 0.0;
            }
        }

        if zorkmids {
            let obj = game_obj_db.remove(things[0]);
            let amount = if let GameObjects::GoldPile(zorkmids) = &obj {
                zorkmids.amount
            } else {
                0
            };

            if amount == 1 {
                state.msg_queue.push_back(Message::info("You pick up a single gold piece."));        
            } else {
                let s = format!("You pick up {} gold pieces.", amount);
                state.msg_queue.push_back(Message::info(&s));                  
            }
            
            let p = game_obj_db.player().unwrap();
            p.purse += amount;
        } else {
            let obj = game_obj_db.remove(things[0]);
            let s = format!("You pick up {}.", obj.get_fullname().with_def_article());
            state.msg_queue.push_back(Message::info(&s));
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
        
        if let Some(answers) = gui.side_pane_menu("Pick up what: (* to get everything)".to_string(), &menu, false) {
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
                        state.msg_queue.push_back(Message::info("You pick up a single gold piece."));
                    } else {
                        let s = format!("You pick up {} gold pieces.", amount);
                        state.msg_queue.push_back(Message::info(&s));                        
                    }
                    game_obj_db.remove(id);
                    game_obj_db.player().unwrap().purse += amount;                   
                } else {
                    let obj = game_obj_db.remove(id);
                    let s = format!("You pick up {}.", obj.get_fullname().with_def_article());
                    state.msg_queue.push_back(Message::info(&s));
                    let p = game_obj_db.player().unwrap();
                    p.add_to_inv(obj);
                }
            }
            return 1.0;
        } else {
            state.msg_queue.push_back(Message::info("Never mind."));
        }
    }

    0.0
}

fn toggle_item(state: &mut GameState, slot: char, game_obj_db: &mut GameObjectDB) -> f32 {
    let player = game_obj_db.player().unwrap();
    let obj = player.inv_item_in_slot(slot).unwrap();
    let obj_id = obj.obj_id();
    let (equipable, item_type, attributes) = if let GameObjects::Item(item) = &obj {
        (item.equipable(), item.item_type, item.attributes)
    } else {
        (false, ItemType::Note, 0)
    };

    if !equipable {
        state.msg_queue.push_back(Message::info("You cannot wear or wield that!"));
        return 0.0;
    }
    
    let mut swapping = false;
    if item_type == ItemType::Weapon {
        if attributes & items::IA_TWO_HANDED > 0 && player.readied_obj_ids_of_type(ItemType::Shield).len() > 0 {
            state.msg_queue.push_back(Message::info("You cannot wield that while using a shield."));
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
            state.msg_queue.push_back(Message::info("You're already wearing armour."));
            return 0.0;             
        }
    } else if item_type == ItemType::Shield {
        let readied = player.readied_obj_ids_of_type(ItemType::Shield);
        if !readied.is_empty() && readied[0] != obj_id {
            state.msg_queue.push_back(Message::info("Yu're already using a shield."));
            return 0.0;
        }

        if let Some((weapon, _)) = player.readied_weapon() {
            if weapon.attributes & items::IA_TWO_HANDED > 0 {
                state.msg_queue.push_back(Message::info("You cannot equip that along with a two-handed weapon!"));
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
        state.msg_queue.push_back(Message::info(&s));
    }
    
    player.calc_gear_effects();

    let readied = player.readied_obj_ids_of_type(ItemType::Weapon);
    if item_type == ItemType::Weapon && readied.is_empty() {
        state.msg_queue.push_back(Message::info("You are now empty handed."));
    } 

    1.0
}

fn toggle_equipment(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {
    let player = game_obj_db.player().unwrap();
    let slots = player.inv_slots_used();
    
    if slots.is_empty() {
        state.msg_queue.push_back(Message::info("You are empty handed."));
        return 0.0;
    }

    let menu = player.inv_menu(2);
    let cost = if let Some(ch) = gui.show_in_side_pane("Equip/unequip which?", &menu) {
        if !slots.contains(&ch) {
            state.msg_queue.push_back(Message::info("You do not have that item!"));
            0.0
        } else {
            toggle_item(state, ch, game_obj_db)
        }
    } else {
        state.msg_queue.push_back(Message::info("Never mind."));
        0.0
    };

    cost
}

fn use_item(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> f32 {
    let sbi = state.curr_sidebar_info(game_obj_db);        
    let player = game_obj_db.player().unwrap();
    let confused = player.confused();
    let slots = player.inv_slots_used();
    
    if slots.is_empty() {
        state.msg_queue.push_back(Message::info("You are empty handed."));
        return 0.0;
    }
    
    let menu = player.inv_menu(1);
    if let Some(ch) = gui.show_in_side_pane("Use which?", &menu) {
        if !slots.contains(&ch) {
            state.msg_queue.push_back(Message::info("You do not have that item!"));
            return 0.0;
        }
        
        let obj = player.inv_item_in_slot(ch).unwrap();
        let obj_id = obj.obj_id();
        let (useable, item_type, consumable, effects, equiped) = if let GameObjects::Item(item) = &obj {
            (item.useable(), item.item_type, item.attributes & IA_CONSUMABLE > 0, item.effects, item.equiped)
        } else {
            (false, ItemType::Weapon, false, 0, false)
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

            if item_type == ItemType::Food {
                state.msg_queue.push_back(Message::info("Om nom nom."));
            }

            if effects > 0 {
                effects::apply_effects(state, 0, game_obj_db, effects);                
            }

            if consumable {
                let player = game_obj_db.player().unwrap();
                player.inv_remove(obj_id);
            }

            return 1.0;
        } else if item_type == ItemType::Weapon && !equiped {
            state.msg_queue.push_back(Message::info("You aren't wielding that."));
        } else if item_type == ItemType::Weapon && equiped {
            if let Some(loc) = gui.select_dir("Use it where?", state, game_obj_db) {
                return if game_obj_db.blocking_obj_at(&loc) { 
                     maybe_fight(state, game_obj_db, loc, gui, confused)
                } else {
                    use_weapon_as_tool(state, game_obj_db, loc)
                };
            } else {
                state.msg_queue.push_back(Message::info("Never mind."));
            }
        } else if !text.is_empty() {
            gui.popup_msg(&desc, &text, Some(&sbi));
        } else {
            state.msg_queue.push_back(Message::info("You don't know how to use that."));
        }       
    } else {
        state.msg_queue.push_back(Message::info("Never mind."));
    }

    0.0
}

fn use_weapon_as_tool(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8)) -> f32 {
    let obstacles = game_obj_db.obstacles_at_loc(loc);
    let web_count = obstacles.iter().filter(|o| o.get_fullname() == "web").count();
    let obstacle_info: Vec<(usize, u8, String)> = obstacles.iter()
                                    .map(|o| (o.obj_id(), o.item_dc, o.get_fullname())).collect();

    for oi in obstacle_info.iter() {
        if oi.2 == "web" {
            let p = game_obj_db.player().unwrap();
            if p.ability_check(Ability::Str) < oi.1 {
                state.msg_queue.push_back(Message::info("You hack away at the web but it remains intact."));
            } else {
                state.msg_queue.push_back(Message::info("You clear out the web."));
                game_obj_db.remove(oi.0);
            }

            return 1.0;
        } else if oi.2 == "mushroom" {
            let p = game_obj_db.player().unwrap();
            let w = p.readied_weapon().unwrap().0;
            if w.dmg_type == DamageType::Slashing {
                state.msg_queue.push_back(Message::info("You chop the mushroom to pieces."));
                for _ in 0..rand::thread_rng().gen_range(1, 4) {
                    let mut m = Item::get_item(game_obj_db, "piece of mushroom").unwrap();
                    m.set_loc(loc);
                    game_obj_db.add(m);
                }
            } else {
                state.msg_queue.push_back(Message::info("You clear out the mushroom."));
            }

            game_obj_db.remove(oi.0);
            
            return 1.0;
        }
    }

    let p = game_obj_db.player().unwrap();
    if let Some(wi) = p.readied_weapon() {
        let s = format!("There's nothing to use your {} for.", wi.1);
        state.msg_queue.push_back(Message::info(&s));
    }

    0.0
}

fn use_light(state: &mut GameState, slot: char, game_obj_db: &mut GameObjectDB) -> (usize, bool) {
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
        state.msg_queue.push_back(Message::info(&s));
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
        Tile::Door(DoorState::Open) | Tile::Door(DoorState::Broken) => state.msg_queue.push_back(Message::info("That door is already open.")),
        Tile::Door(DoorState::Closed) => {
            state.msg_queue.push_back(Message::info("You open the door."));
            state.map.insert(loc, map::Tile::Door(DoorState::Open));
        },
        Tile::Door(DoorState::Locked) => state.msg_queue.push_back(Message::info("That door is locked.")),
        _ => state.msg_queue.push_back(Message::info("You cannot open that.")),
    }        
}

fn do_close(state: &mut GameState, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB) {
    let tile = &state.map[&loc];
    match tile {
        Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) => state.msg_queue.push_back(Message::info("That door is already closed.")),
        Tile::Door(DoorState::Open) => {
            if !game_obj_db.things_at_loc(loc).is_empty() {
                state.msg_queue.push_back(Message::info("There's something in the way!"));             
            } else {
                state.msg_queue.push_back(Message::info("You close the door."));
                state.map.insert(loc, map::Tile::Door(DoorState::Closed));
            }
        },
        Tile::Door(DoorState::Broken) => state.msg_queue.push_back(Message::info("That door is broken!")),
        _ => state.msg_queue.push_back(Message::info("You cannot closed that!")),
    }        
}

fn take_stairs(state: &mut GameState, game_obj_db: &mut GameObjectDB, down: bool) -> f32 {
    let player_loc = game_obj_db.get(0).unwrap().get_loc();
    let tile = &state.map[&player_loc];
    
    if down {
        let cost = if *tile == map::Tile::Portal {
            state.msg_queue.push_back(Message::info("You enter the beckoning portal."));
            game_obj_db.set_to_loc(0, (player_loc.0, player_loc.1, player_loc.2 + 1));
            1.0
        } else if *tile == map::Tile::StairsDown {
            state.msg_queue.push_back(Message::info("You brave the stairs downward."));
            game_obj_db.set_to_loc(0, (player_loc.0, player_loc.1, player_loc.2 + 1));
            1.0
        } else {
            state.msg_queue.push_back(Message::info("You cannot do that here."));            
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
            state.msg_queue.push_back(Message::info("You climb the stairway."));
            game_obj_db.set_to_loc(0, (player_loc.0, player_loc.1, player_loc.2 - 1));
            
            if player_loc.2 == 1 {
                state.msg_queue.push_back(Message::info("Fresh air!"));
            }

            return 1.0;
        } else {
            state.msg_queue.push_back(Message::info("You cannot do that here."));
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
                state.msg_queue.push_back(Message::info("You are shoved out of the way by the falling gate!"));
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
                state.msg_queue.push_back(Message::new(npc_id, start_loc, &s, ""));
                
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

fn maybe_fight(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), gui: &mut GameUI, confused: bool) -> f32 {
    if let Some(npc_id) = game_obj_db.npc_at(&loc) {
        let npc = game_obj_db.get_mut(npc_id).unwrap();
        let (npc_name, attitude) = if let GameObjects::NPC(other) = npc {
            (other.npc_name(false), other.attitude)
        } else {
            ("".to_string(), Attitude::Indifferent)
        };

        if confused {
            battle::player_attacks(state, npc_id, game_obj_db);
            return 1.0;
        }
        
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
                state.msg_queue.push_back(Message::new(npc_id, loc, &s, "Someone is in your way!"));
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

fn check_for_obstacles(state: &mut GameState, game_obj_db: &mut GameObjectDB, obj_id: usize, loc: (i32, i32, i8)) -> f32 {
    let obstacles = game_obj_db.obstacles_at_loc(loc);
    let web_count = obstacles.iter().filter(|o| o.get_fullname() == "web").count();
    let obstacle_info: Vec<(usize, u8, String)> = obstacles.iter()
                                    .map(|o| (o.obj_id(), o.item_dc, o.get_fullname())).collect();

    for oi in obstacle_info.iter() {
        if oi.2 == "web" {
            let agent = game_obj_db.as_person(obj_id).unwrap();

            // I dunno if one spider can get caught in another spider's web but in my game, if you can spin a web
            // you won't get stuck in any web
            if agent.attributes() & MA_WEBSLINGER > 0 {
                return 0.0;
            }

            if agent.ability_check(Ability::Str) < oi.1 {
                let msg = util::format_msg(obj_id, "to be", "held fast by the web!", game_obj_db);
                state.msg_queue.push_back(Message::new(obj_id, loc, &msg, ""));
                return 1.0;
            } else {
                game_obj_db.remove(oi.0);

                // Are they free or is there still more webbing?
                if web_count > 1 {
                    let msg = util::format_msg(obj_id, "tear", "through some of the webbing!", game_obj_db);
                    state.msg_queue.push_back(Message::new(obj_id, loc, &msg, ""));
                    return 1.0;
                } else {
                    let msg = util::format_msg(obj_id, "tear", "through the web!", game_obj_db);
                    state.msg_queue.push_back(Message::new(obj_id, loc, &msg, ""));                    
                }            
            }
        } else if oi.2 == "rubble" {
            let agent = game_obj_db.as_person(obj_id).unwrap();
            if agent.ability_check(Ability::Dex) <= 12 {            
                let msg = util::format_msg(obj_id, "stumble", "over the rubble!", game_obj_db);
                state.msg_queue.push_back(Message::new(obj_id, loc, &msg, ""));
                return 1.0;
            }
        }
    }

    0.0
}

pub fn take_step(state: &mut GameState, game_obj_db: &mut GameObjectDB, obj_id: usize, start_loc: (i32, i32, i8), next_loc: (i32, i32, i8)) -> (f32, bool) {    
    let cost = check_for_obstacles(state, game_obj_db, obj_id, start_loc);
    if cost > 0.0 { return (cost, false); }

    game_obj_db.set_to_loc(obj_id, next_loc);
    
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
        game_obj_db.set_to_loc(obj_id, sq);                
        if obj_id == 0 {
            state.msg_queue.push_back(Message::new(0, sq, "You have a feeling of vertigo!", "You have a feeling of vertigo!"));
        } else {
            let npc = game_obj_db.npc(obj_id).unwrap();
            let s = format!("{} disappears!", npc.npc_name(false).capitalize());
            state.msg_queue.push_back(Message::new(0, sq, &s, ""));
        }
    }

    return (1.0, true);
}

fn do_move(state: &mut GameState, game_obj_db: &mut GameObjectDB, dir: &str, gui: &mut GameUI) -> f32 {
    let player = game_obj_db.player().unwrap();
    let confused = player.confused();

    // if the player is confused, they walk in their intended direction 1/5 of the time, otherwise
    // they stagger in a random direction.
    let mv = if confused && rand::thread_rng().gen_range(0.0, 1.0) < 0.2 {
        state.msg_queue.push_back(Message::info("You stagger."));
        let roll = rand::thread_rng().gen_range(0, 8);
        match roll {
            0 => get_move_tuple("n"),
            1 => get_move_tuple("e"),
            2 => get_move_tuple("s"),
            3 => get_move_tuple("w"),
            4 => get_move_tuple("ne"),
            5 => get_move_tuple("nw"),
            6 => get_move_tuple("se"),
            _ => get_move_tuple("sw"),
        }        
    } else {
        get_move_tuple(dir)
    };

    let start_loc = game_obj_db.get(0).unwrap().get_loc();
    let start_tile = state.map[&start_loc];
    let next_loc = (start_loc.0 + mv.0, start_loc.1 + mv.1, start_loc.2);
    let tile = state.map[&next_loc].clone();
    
    if game_obj_db.blocking_obj_at(&next_loc) {
        return maybe_fight(state, game_obj_db, next_loc, gui, confused);
    } else if tile.passable() {
        let (cost, moved) = take_step(state, game_obj_db, 0, start_loc, next_loc);

        if !moved {
            return cost;
        }

        match tile {
            Tile::Water => state.msg_queue.push_back(Message::new(0, next_loc, "You splash in the shallow water.", "You splash in the shallow water.")),
            Tile::DeepWater => {
                if start_tile != Tile::DeepWater {
                    state.msg_queue.push_back(Message::new(0, next_loc, "You wade into the flow.", "You wade into the flow."))
                }
            },
            Tile::Well => state.msg_queue.push_back(Message::new(0, next_loc, "There is a well here.", "There is a well here.")),
            Tile::Lava => state.msg_queue.push_back(Message::new(0, next_loc, "MOLTEN LAVA!", "MOLTEN LAVA!")),
            Tile::FirePit => state.msg_queue.push_back(Message::new(0, next_loc, "You've stepped in the fire!", "You've stepped in the fire!")),
            Tile::OldFirePit(n) => state.msg_queue.push_back(Message::new(0, next_loc, firepit_msg(n), "You feel the remains of an old firepit.")),
            Tile::Portal => state.msg_queue.push_back(Message::new(0, next_loc, "Where could this lead?", "")),
            Tile::Shrine(stype) => {
                match stype {
                    ShrineType::Woden => state.msg_queue.push_back(Message::new(0, next_loc, "A shrine to Woden.", "")),
                    ShrineType::Crawler => state.msg_queue.push_back(Message::new(0, next_loc, "The misshappen altar makes your skin crawl.", "You have a feeling of unease.")),
                }
            },
            _ => {
                if state.aura_sqs.contains(&next_loc) && !state.aura_sqs.contains(&start_loc) {
                    state.msg_queue.push_back(Message::new(0, next_loc, "You feel a sense of peace.", "You feel a sense of peace."))
                }
            },            
        }

        let items = game_obj_db.descs_at_loc(&next_loc);
        let item_count = items.len();                        
        if item_count == 1 {
            // Bit of a hacky way to check if the lone item on the square is actually a stack
            // and use correct grammar
            let s1 = if items[0].chars().next().unwrap().is_digit(10) {
                format!("There are {} here.", items[0])
            } else {
                format!("There is {} here.", items[0])
            };
            
            let s2 = format!("You feel {} here.", items[0]);
            state.msg_queue.push_back(Message::new(0, next_loc, &s1, &s2));
        } else if item_count == 2 {
            let s = format!("There are {} and {} here.", items[0], items[1]);
            state.msg_queue.push_back(Message::new(0, next_loc, &s, "There is something on the ground."));            
        } else if item_count > 2 {
            state.msg_queue.push_back(Message::new(0, next_loc, "There are several items here.", "You feel several items on the ground."));
        }
        
        return cost;
    } else if tile == Tile::Door(DoorState::Closed) {
        // Bump to open doors. I might make this an option later
        do_open(state, next_loc);
        return 1.0;
    } else if tile == Tile::Door(DoorState::Locked) {  
        state.msg_queue.push_back(Message::new(0, next_loc, "You door is locked.", "The door is locked."));
        return 1.0;
    } else if tile == Tile::Gate(DoorState::Closed) || tile == Tile::Gate(DoorState::Locked) {
        state.msg_queue.push_back(Message::new(0, next_loc, "A portcullis bars your way.", "A portcullis bars your way."));        
    } else  {
        state.msg_queue.push_back(Message::new(0, next_loc, "You cannot go that way.", "You cannot go that way."));
    }

    0.0
}

// I don't know how real noise works but when I want to alert monsters to something noisy a player did, I'm
// going to floodfill out to a certain radius. (Which closed doors muffling the noise)
// Another semi-duplicate implementation of floodfill but this one does work a little differently than the others.
fn floodfill_noise(state: &mut GameState, game_obj_db: &mut GameObjectDB, centre: (i32, i32, i8), radius: u8, _actor_id: usize) {
    // find start point
    let mut q = VecDeque::new();
    let mut visited = HashSet::new();
    let start = (centre, 0);

    q.push_front(start);
    while !q.is_empty() {
        let pt = q.pop_front().unwrap();
        let mut distance = pt.1;
        if visited.contains(&pt) {
            continue;
        }
        
        visited.insert(pt);

        match state.map[&pt.0] {
            Tile::WoodWall | Tile::Wall => { continue; },
            Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) | Tile::Window(_) => distance += 4,
            _ => { distance += 1 },
        }
        
        let loc = pt.0;
        for adj in util::ADJ.iter() {
            let n = (loc.0 + adj.0, loc.1 + adj.1, loc.2);
            if distance > radius || !state.map.contains_key(&n) || state.map[&n] == Tile::WoodWall || state.map[&n] == Tile::Wall   {
                continue;
            }

            q.push_back((n, distance));
        }
    }

    // Now we have to alert/wake up any monsters in the visited sqs
    for loc in visited {
        if let Some(npc_id) = game_obj_db.npc_at(&loc.0) {
            npc::heard_noise(npc_id, centre, state, game_obj_db);
        }
    }    
}

fn bash(state: &mut GameState, loc: (i32, i32, i8), game_obj_db: &mut GameObjectDB) -> f32 {
    let tile = state.map[&loc];

    if tile == Tile::Door(DoorState::Locked) || tile == Tile::Door(DoorState::Closed) {
        floodfill_noise(state, game_obj_db, loc, 10, 0);
        let player = game_obj_db.player().unwrap();
        if player.ability_check(Ability::Str) > 17 {
            state.msg_queue.push_back(Message::info("BAM! You knock down the door!"));
            state.map.insert(loc, Tile::Door(DoorState::Broken));           
        } else {
            state.msg_queue.push_back(Message::info("The door holds firm."));
        }        
    } else if tile == Tile::Wall || tile == Tile::WoodWall {
        state.msg_queue.push_back(Message::info("Ouch! You slam yourself into the wall!"));
        let player = game_obj_db.player().unwrap();
        player.damaged(state, rand::thread_rng().gen_range(1, 6), battle::DamageType::Bludgeoning, 0, "a wall");
    } else if  game_obj_db.blocking_obj_at(&loc) {
        // I don't yet have blocking_objs that aren't creatures...
        battle::knock_back(state, game_obj_db, loc);
    } else {
        // I should perhaps move them?
        state.msg_queue.push_back(Message::info("You flail about in a silly fashion."));
    }
    
    1.0
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
                    gui.popup_msg(&npc.npc_name(true).capitalize(), &line, Some(&sbi));
                }
            },
        }           
    } else {
        if let Tile::Door(_) = state.map[&loc] {
            state.msg_queue.push_back(Message::info("The door is ignoring you."));
        } else {
            state.msg_queue.push_back(Message::info("Oh no, talking to yourself?"));            
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
    let menu = p.inv_menu(0);
    let purse = p.purse;

    let money = if purse == 1 {
        String::from("$) a single zorkmid to your name")
    } else {
        let s = format!("$) {} gold pieces", purse);
        s
    };

    if menu.is_empty() && purse == 0 {
        state.msg_queue.push_back(Message::info("You are empty handed."));
    } else {
        let mut m: Vec<(String, bool)> = menu.iter().map(|m| (m.0.to_string(), m.1)).collect();        
        if purse > 0 {
            m.insert(0, (money.to_string(), true));
        }
        
        gui.show_in_side_pane("You are carrying:", &m);
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
                state.msg_queue.push_back(Message::info("Uhh the wilderness is too big to dump."));
            } else {
                dump_level(state, player_loc.2);
            }
        } else if pieces.len() != 2 {
            state.msg_queue.push_back(Message::info("Invalid wizard command."));
        } else if pieces[0] == "turn" {
            let num = pieces[1].parse::<u32>();
            match num {
                Ok(v) => state.turn = v,
                Err(_) => state.msg_queue.push_back(Message::info("Invalid wizard command.")),
            }
        } else {
            state.msg_queue.push_back(Message::info("Invalid wizard command."));
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
fn fov_to_tiles(state: &mut GameState, game_obj_db: &GameObjectDB, visible: &[((i32, i32, i8), bool)], player_loc: (i32, i32, i8)) -> [(map::Tile, bool); FOV_HEIGHT * FOV_WIDTH] {
    let mut v_matrix = [(map::Tile::Blank, false); FOV_HEIGHT * FOV_WIDTH];
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
                
                // I will probably eventually get rid of having auras light squares at some point
                if state.aura_sqs.contains(&vis.0) && state.map[&vis.0] == Tile::StoneFloor {
                    Tile::ColourFloor(display::LIGHT_BLUE)
                } else if state.lit_sqs.contains_key(&vis.0) {
                    let colour = state.lit_sqs[&vis.0];

                    // Fuckery here because I want a torch to colour only floors squares, but for things 
                    // like mushrooms, I later decided I wanted them to light up stone walls as well.
                    if colour == display::WHITE {
                        match state.map[&vis.0] {
                            Tile::StoneFloor => Tile::ColourFloor(display::YELLOW),
                            Tile::Trigger => Tile::ColourFloor(display::YELLOW_ORANGE),
                            _ => state.map[&vis.0],
                        }
                    } else {
                        match state.map[&vis.0] {
                            Tile::StoneFloor => Tile::ColourFloor(colour),
                            Tile::Trigger => Tile::ColourFloor(colour),
                            Tile::Wall => Tile::LitWall(colour),
                            _ => state.map[&vis.0],
                        }
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
        state.msg_queue.push_back(Message::info("Oh no! You have died!"));
    } else {
        let s = format!("Oh no! You have been killed by {}!", msg);
        state.msg_queue.push_back(Message::info(&s));        
    }
    
    let s = format!("Farewell, {}.", game_obj_db.get(0).unwrap().get_fullname());
    state.msg_queue.push_back(Message::info(&s));
    let sbi = state.curr_sidebar_info(game_obj_db);

    let mut msgs = VecDeque::new();
    while !state.msg_queue.is_empty() {
        let msg = state.msg_queue.pop_front().unwrap();
        msgs.push_back(msg.text);
    }
    gui.update(&mut msgs, Some(&sbi));
    gui.pause_for_more();
}

// Herein lies the main game loop
fn run_game_loop(gui: &mut GameUI, state: &mut GameState, game_obj_db: &mut GameObjectDB, dialogue: &DialogueLibrary, monster_fac: &MonsterFactory) -> Result<(), ExitReason> {    
    update_view(state, game_obj_db, gui);
    
    loop {
        state.animation_pause = false;
        let mut curr_energy = 0.0;
        if let Some(GameObjects::Player(player)) = game_obj_db.get(0) {
            curr_energy = player.energy;
        }
        
        let mut skip_turn = false;
        let mut effects: u128 = 0;
        while curr_energy >= 1.0 {
            gui.clear_msg_buff();
            
            // Here we look for any statuses that should have effects at the start of a player's turn.
            // After their turn we'll check to see if the statuses have ended.
            let p = game_obj_db.player().unwrap();
            for status in p.get_statuses().unwrap().iter() {
                match status {
                    Status::PassUntil(_) => skip_turn = true,
                    Status::RestAtInn(_) => skip_turn = true,
                    Status::Paralyzed(_) => skip_turn = true,
                    Status::WeakVenom(_) => effects |= effects::EF_WEAK_VENOM,
                    Status::BlindUntil(_) => { }, // blindness is handled when we check the player's vision radius
                    _ => { },
                }                
            }
            
            if effects > 0 {
                effects::apply_effects(state, 0, game_obj_db, effects);
                
                if !state.msg_queue.is_empty() {
                    update_view(state, game_obj_db, gui);
                }                
            }

            check_event_queue(state, game_obj_db, gui)?;
            
            let cmd = if skip_turn {
                Cmd::Pass
            } else  {
                gui.get_command(&state, game_obj_db)
            };

            let mut energy_cost = 0.0;
            match cmd {
                Cmd::Bash(loc) => energy_cost = bash(state, loc, game_obj_db),
                Cmd::Chat(loc) => energy_cost = chat_with(state, gui, loc, game_obj_db, dialogue),
                Cmd::Close(loc) => {
                    do_close(state, loc, game_obj_db);
                    energy_cost = 1.0;
                },
                Cmd::Down => energy_cost = take_stairs(state, game_obj_db, true),
                Cmd::DropItem => energy_cost = drop_item(state, game_obj_db, gui),  
                Cmd::Move(dir) => energy_cost = do_move(state, game_obj_db, &dir, gui),
                Cmd::MsgHistory => gui.show_message_history(),
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
                if !skip_turn || !state.msg_queue.is_empty() {
                    update_view(state, game_obj_db, gui);
                }
            }
        }
        
        check_event_queue(state, game_obj_db, gui)?;
        
        // There are moments where I want to update the view and pause very briefly
        // to show some effect to the player. (Otherwise, eg, if you bash a monster
        // backwards and they immediately step toward you, it would have been too fast
        // to see anything)
        if state.animation_pause {
            update_view(state, game_obj_db, gui);
            ::std::thread::sleep(Duration::new(0, 75_000_000u32));
        }

        let p = game_obj_db.player().unwrap();
        effects::check_statuses(p, state);

        game_obj_db.do_npc_turns(state);
        game_obj_db.update_listeners(state, EventType::Update);
        game_obj_db.update_listeners(state, EventType::EndOfTurn);
        
        check_event_queue(state, game_obj_db, gui)?;

        let p = game_obj_db.player().unwrap();
        p.energy += p.energy_restore;
        if state.turn % 25 == 0 {
             p.recover();
        }

        state.turn += 1;

        if !skip_turn || !state.msg_queue.is_empty() {
            update_view(state, game_obj_db, gui);
        }

        // if skip_turn {
        //     // Just a little pause if skipping turns so the CPU doesn't go crazy
        //     ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
        // }
    }
}

fn check_event_queue(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> Result<(), ExitReason> {
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
                p.level_up();
                let level = p.level;
                let s = format!("Welcome to level {}!", level);
                state.msg_queue.push_back(Message::info(&s));                
            },
            (EventType::DeathOf(npc_id), _, _, _) => {
                game_obj_db.update_listeners(state, EventType::DeathOf(npc_id));
            },
            _ => { },
        }                
    }

    Ok(())
}

fn update_view(state: &mut GameState, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) {    
    let player = game_obj_db.player().unwrap();
    let player_loc = player.get_loc();
    player.calc_vision_radius(state, player_loc);
    let player_vr = player.vision_radius;
    
    //let _fov_start = Instant::now();
    let visible = fov::visible_sqs(state, player_loc, player_vr, false);
    state.curr_visible = visible.iter()
                                .filter(|sq| sq.1)
                                .map(|sq| sq.0)
                                .collect();
    
    gui.v_matrix = fov_to_tiles(state, game_obj_db, &visible, player_loc);        
    //let _fov_duration = _fov_start.elapsed();
    //println!("Player fov: {:?}", fov_duration);
    
    //let write_screen_start = Instant::now();
    let sbi = state.curr_sidebar_info(game_obj_db);

    // this is semi-temporary code. Eventually msg_queue will replace msg_buff
    // I don't know about the distance calculation for noise. A floodfill like I'm doing
    // to alert monsters is probably better but more complicated.
    let mut msgs = VecDeque::new();
    while !state.msg_queue.is_empty() {
        let msg = state.msg_queue.pop_front().unwrap();
        if state.curr_visible.contains(&msg.loc) || (msg.obj_id == 0 && msg.loc == (-1, -1, -1)) {
            msgs.push_back(msg.text);
        } else if !msg.alt_text.is_empty() && util::distance(player_loc.0, player_loc.1, msg.loc.0, msg.loc.1) < 12.0 {
            msgs.push_back(msg.alt_text);            
        }
    }

    gui.update(&mut msgs, Some(&sbi));
    //let write_screen_duration = write_screen_start.elapsed();
    //println!("Time for write_screen(): {:?}", write_screen_duration); 
}

fn fetch_config_options() -> ConfigOptions {
    match fs::read_to_string("options") {
        Ok(contents) => {
            let mut co = ConfigOptions { font_size: 24, sm_font_size: 18 };
            let lines = contents.split('\n').collect::<Vec<&str>>();

            for line in lines.iter() {
                let pieces = line.split('=').collect::<Vec<&str>>();
                if pieces[0] == "font_size" {
                    co.font_size = pieces[1].parse::<u16>().unwrap()
                }
                if pieces[0] == "sm_font_size" {
                    co.sm_font_size = pieces[1].parse::<u16>().unwrap();
                }
            }

            co
        },
        Err(_) => ConfigOptions { font_size: 24, sm_font_size: 18 },
    }
    //let contents = fs::read_to_string("options")
    //    .expect("Unable to find building templates file!");
    //let lines = contents.split('\n').collect::<Vec<&str>>();
}

fn main() {
    let opts = fetch_config_options();
    
    // It bugs me aesthetically that I can't move creating the font contexts into the 
    // constructor for GameUI. But the borrow check loses its shit whenever I try.
    let ttf_context = sdl2::ttf::init()
        .expect("Error creating ttf context on start-up!");
    let font_path: &Path = Path::new("DejaVuSansMono.ttf");
    let font = ttf_context.load_font(font_path, opts.font_size)
        .expect("Error loading game font!");
    let sm_font = ttf_context.load_font(font_path, opts.sm_font_size)
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
            state.msg_queue.push_back(Message::info(&msg));
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
        
        state.msg_queue.push_back(Message::info("Welcome, adventurer."));        
    }
    
    // for _ in 0..20 {
    //     println!("{}", MonsterFactory::pick_monster_level(10));
    // }

    match run_game_loop(&mut gui, &mut state, &mut game_obj_db, &dialogue_library, &mf) {
        Ok(_) => println!("Game over I guess? Probably the player won?!"),
        //Err(ExitReason::Save) => save_msg(&mut state, &mut gui),
        //Err(ExitReason::Quit) => quit_msg(&mut state, &mut gui),
        //Err(ExitReason::Win) => victory_msg(&mut state, &mut gui),
        //Err(ExitReason::Death(src)) => death(&mut state, src, &mut gui),
        Err(_) => println!("okay bye"),
    }
}
