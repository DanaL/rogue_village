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
mod dialogue;
mod display;
mod dungeon;
mod game_obj;
mod fov;
mod items;
mod map;
mod pathfinding;
mod player;
mod town;
mod util;
mod wilderness;
mod world;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::prelude::*;
use std::fs;
use std::fs::File;
use std::path::Path;

//use std::time::{Duration, Instant};

use rand::{Rng, thread_rng};
use serde::{Serialize, Deserialize};

use dialogue::DialogueLibrary;
use display::{GameUI, SidebarInfo, WHITE, YELLOW};
use game_obj::{GameObject, GameObjects, GameObjType, GOForSerde};
use items::{GoldPile, Item, ItemType};
use map::{Tile, DoorState};
use player::Player;
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

#[derive(Debug, Hash, Eq, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub enum EventType {
    EndOfTurn,
    LightExpired,
    TakeTurn,
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
	FireGun,
	Reload,
	Search,
	Read,
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
    player_loc: (i32, i32, i8),
    world_info: WorldInfo,
    tile_memory: HashMap<(i32, i32, i8), Tile>,
    lit_sqs: HashSet<(i32, i32, i8)>, // by light sources independent of player
    next_obj_id: usize,    
}

impl GameState {
    pub fn init(map: Map, world_info: WorldInfo) -> GameState {
        let state = GameState {
            msg_buff: VecDeque::new(),
            msg_history: VecDeque::new(),
			map: map,
            turn: 0,
            player_loc: (-1, -1, -1),
            world_info: world_info,
            tile_memory: HashMap::new(),
            lit_sqs: HashSet::new(),
            next_obj_id: 0,            
        };

        state
    }

    pub fn add_to_msg_history(&mut self, msg: &str) {
        if self.msg_history.len() == 0 || msg != self.msg_history[0].0 {
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

		if msg.len() > 0 {
			self.add_to_msg_history(msg);
		}
	}

    pub fn curr_sidebar_info(&self, player: &Player) -> SidebarInfo {
        let weapon = if let Some(w) = &player.readied_weapon {
            w.name.capitalize()    
        } else {
            String::from("Empty handed")
        };

		SidebarInfo::new(player.name.to_string(), player.curr_hp, player.max_hp, self.turn, player.ac,
         player.purse, weapon)
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
			s.push_str(")");
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

fn serialize_game_data(state: &GameState, game_objs: &GameObjects, player: &Player) {
    let go = GOForSerde::convert(game_objs);
    let game_data = (state, go, player);
    let serialized = serde_yaml::to_string(&game_data).unwrap();

    let filename = calc_save_filename(&player.name);

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

fn load_save_game(player_name: &str) -> Result<(GameState, GameObjects, Player), serde_yaml::Error> {
    let filename = calc_save_filename(player_name);
    let blob = fs::read_to_string(filename).expect("Error reading save file");
    let game_data: (GameState, GOForSerde, Player) = serde_yaml::from_str(&blob)?;

    let game_objs = GOForSerde::revert(game_data.1);
    Ok((game_data.0, game_objs, game_data.2))
}

fn fetch_saved_data(player_name: &str) -> Option<(GameState, GameObjects, Player)> {    
    match load_save_game(player_name) {
        Ok(gd) => Some(gd),
        Err(_) => None,
    }
}

fn save_and_exit(state: &GameState, game_objs: &GameObjects, player: &Player, gui: &mut GameUI) -> Result<(), ExitReason> {
    let sbi = state.curr_sidebar_info(player);
    match gui.query_yes_no("Save and exit? (y/n)", Some(&sbi)) {
        'y' => {
            serialize_game_data(state, game_objs, player);
            Err(ExitReason::Save)
        },
        _ => Ok(()),
    }
}

fn who_are_you(gui: &mut GameUI) -> String {
    loop {
        match gui.query_user("Who are you?", 15, None) {
            Some(name) => {
                return name;
            },
            None => { },
        }
    }
}

fn start_new_game(state: &GameState, game_objs: &mut GameObjects, gui: &mut GameUI, player_name: String) -> Option<Player> {
    let mut menu = vec!["Welcome adventurer, please choose your role in RogueVillage:"];
	menu.push("");
	menu.push("  (a) Human Warrior - a doughty fighter who lives by the sword and...well");
    menu.push("                      hopefully just that first part.");
	menu.push("");
	menu.push("  (b) Human Rogue - a quick, sly adventurer who gets by on their light step");
    menu.push("                    and fast blade.");
	
    if let Some(answer) = gui.menu_picker(&menu, 2, true, true) {
        if answer.contains(&0) {
            let mut player = Player::new_warrior(game_objs, player_name);
            player.location = pick_player_start_loc(&state);
            return Some(player);
        } else {
            let mut player = Player::new_rogue(game_objs, player_name);
            player.location = pick_player_start_loc(&state);
            return Some(player);
        }
    }

    None
}

fn fetch_player(state: &GameState, game_objs: &mut GameObjects,  gui: &mut GameUI, player_name: String) -> Option<Player> {
    // Of course eventually this is where I'll check for a saved game and load
    // it if one exists.
    start_new_game(state, game_objs, gui, player_name)
}

fn drop_zorkmids(state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, gui: &mut GameUI) {
    // I need to add a check in GameObjects to see if there is an existing pile of gold I can merge with    
    if player.purse == 0 {
        state.write_msg_buff("You have no money!");
        return;
    }

    let sbi = state.curr_sidebar_info(player);
    let amt = gui.query_natural_num("How much?", Some(&sbi)).unwrap();
    if amt == 0 {
        state.write_msg_buff("Never mind.");                
    } else {
        if amt >= player.purse {
            state.write_msg_buff("You drop all your money.");
            let zorkmids = GoldPile::new(game_objs.next_id(), player.purse, player.location);
            game_objs.add(Box::new(zorkmids));
            player.purse = 0;
        } else if amt > 1 {
            let s = format!("You drop {} gold pieces.", amt);
            state.write_msg_buff(&s);
            let zorkmids = GoldPile::new(game_objs.next_id(), amt, player.location);
            game_objs.add(Box::new(zorkmids));
            player.purse -= amt;
        } else {
            state.write_msg_buff("You drop a gold piece.");
            let zorkmids = GoldPile::new(game_objs.next_id(), amt, player.location);
            game_objs.add(Box::new(zorkmids));
            player.purse -= 1;
        }
        state.turn += 1;
    }
}

// Pretty similar to item_hits_grounds() but this keeps calculating the message to display simpler
fn stack_hits_ground(state: &mut GameState, stack: &Vec<Item>, loc: (i32, i32, i8), game_objs: &mut GameObjects) {
    let s = format!("You drop {}", stack[0].get_fullname().with_def_article().pluralize());
    for item in stack {
        let mut obj = item.clone();
        obj.equiped = false;
        obj.location = loc;
        game_objs.add(Box::new(obj));
    }
    state.write_msg_buff(&s);
}

fn item_hits_ground(state: &mut GameState, item: &Item, loc: (i32, i32, i8), game_objs: &mut GameObjects) {
    let mut obj = item.clone();
    obj.equiped = false;
    obj.location = loc;
    let s = format!("You drop {}.", &obj.get_fullname().with_def_article());                
    state.write_msg_buff(&s);
    game_objs.add(Box::new(obj));
}

fn drop_stack(state: &mut GameState, game_objs: &mut GameObjects, loc: (i32, i32, i8), slot: char, count: u32) {
    match game_objs.inv_remove_from_slot(slot, count) {
        Ok(mut pile) => {
            if pile.len() == 1 {
                let item = pile.remove(0);
                item_hits_ground(state, &item, loc, game_objs);
                state.turn += 1;
            } else if pile.len() > 1 {
                stack_hits_ground(state, &pile, loc, game_objs);
                state.turn += 1;
            } else {
                state.write_msg_buff("Nevermind.");
            }
        },
        Err(msg) => state.write_msg_buff(&msg),
    }        
}

fn drop_item(state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, gui: &mut GameUI) {    
    if player.purse == 0 && game_objs.descs_at_loc(PLAYER_INV).len() == 0 {
    	state.write_msg_buff("You are empty handed.");
		return;
    }
	
    let sbi = state.curr_sidebar_info(player);
    if let Some(ch) = gui.query_single_response("Drop what?", Some(&sbi)) {
        if ch == '$' {
            drop_zorkmids(state, player, game_objs, gui);
        } else {
            let count = game_objs.inv_count_at_slot(ch);
            if count == 0 {
                state.write_msg_buff("You do not have that item.");
            } else if count > 1 {
                match gui.query_natural_num("Drop how many?", Some(&sbi)) {
                    Some(v) => drop_stack(state, game_objs, player.location, ch, v),
                    None => state.write_msg_buff("Nevermind"),
                }
            } else {
                let result = game_objs.inv_remove_from_slot(ch, 1);
                match result {
                    Ok(items) => {
                        item_hits_ground(state, &items[0], player.location, game_objs);
                        state.turn += 1;
                    },
                    Err(msg) => state.write_msg_buff(&msg),
                }
                
            }
        }
    } else {
        state.write_msg_buff("Nevermind.");            
    }
    
    player.calc_ac(game_objs);
    player.set_readied_weapon(game_objs);
}

fn pick_up(state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, gui: &mut GameUI) {
    let things = game_objs.things_at_loc(player.location);

    if things.len() == 0 {
        state.write_msg_buff("There is nothing here.");
        return;
    } else if things.len() == 1 {
        if things[0].get_type() == GameObjType::Zorkmids {
            let obj_id = things[0].get_object_id();
            let zorkmids = game_objs.get(obj_id).as_zorkmids().unwrap();
            if zorkmids.amount == 1 {
                state.write_msg_buff(&"You pick up a single gold piece.");
            } else {
                let s = format!("You pick up {} gold pieces.", zorkmids.amount);
                state.write_msg_buff(&s);
            }
            player.purse += zorkmids.amount;
        } else if things[0].get_type() == GameObjType::Item {
            let obj_id = things[0].get_object_id();
            let item = game_objs.get(obj_id).as_item().unwrap();
            let s = format!("You pick up {}.", item.get_fullname().with_def_article());
            state.write_msg_buff(&s);
            game_objs.add_to_inventory(item);            
        }
        
        state.turn += 1;
    } else {
        let mut m = game_objs.get_pickup_menu(player.location);
        let mut answer_key = HashMap::new();
        let mut menu = Vec::new();
        for j in 0..m.len() {
            if m[j].0.contains("gold piece") {
                menu.push((m[j].0.to_string(), '$'));
                answer_key.insert('$', m[j].1);
                m.remove(j);
                break;
            }
        }
        for j in 0..m.len() {
            let ch = (j as u8 + 'a' as u8) as char;
            menu.push((m[j].0.to_string(), ch));
            answer_key.insert(ch, m[j].1);
        }
        
        if let Some(answers) = gui.menu_picker2("Pick up what: (* to get everything)".to_string(), &menu, false, true) {
            let picks: Vec<usize> = answers.iter().map(|a| answer_key[a]).collect();
            for id in picks {
                let obj = game_objs.get(id);
                if let Some(pile) = obj.as_zorkmids() {
                    player.purse += pile.amount;
                    if pile.amount == 1 {
                        state.write_msg_buff("You pick up a single gold piece.");
                    } else {
                        let s = format!("You pick up {} gold pieces.", pile.amount);
                        state.write_msg_buff(&s);
                    }
                } else {
                    let s = format!("You pick up {}.", obj.get_fullname().with_def_article());
                    state.write_msg_buff(&s);
                    game_objs.add_to_inventory(obj.as_item().unwrap());
                }
            }
            state.turn += 1;
        } else {
            state.write_msg_buff("Nevermind.");
        }
    }
}

fn toggle_item(state: &mut GameState, game_objs: &mut GameObjects, item: Item, player: &mut Player) {
    if !item.equipable() {
        state.write_msg_buff("You cannot wear or wield that!");
        return;
    }

    let mut swapping = false;
    if item.item_type == ItemType::Weapon {
        if let Some(w) = game_objs.readied_weapon() {
            if w.object_id != item.object_id {
                swapping = true;
                // unequip the existing weapon
                let mut other = game_objs.get(w.object_id).as_item().unwrap();
                other.equiped = false;
                game_objs.add_to_inventory(other);
            }
        }
    } else if item.item_type == ItemType::Armour {
        if let Some(a) = game_objs.readied_armour() {
            if a.object_id != item.object_id {
                state.write_msg_buff("You're already wearing armour.");
                return;
            }
        }
    }

    // Alright, so at this point we can toggle the item in the slot.
    let mut obj = game_objs.get(item.get_object_id()).as_item().unwrap();
    obj.equiped = !obj.equiped;
    
    let mut s = String::from("You ");
    if swapping {
        s.push_str("are now wielding ");
    } else if obj.equiped {
        s.push_str("equip ");
    } else {
        s.push_str("unequip ");
    }
    s.push_str(&obj.get_fullname().with_def_article());
    s.push('.');
    state.write_msg_buff(&s);

    game_objs.add_to_inventory(obj);

    player.calc_ac(game_objs);
    player.set_readied_weapon(game_objs);

    if item.item_type == ItemType::Weapon && game_objs.readied_weapon() == None {
        state.write_msg_buff("You are now empty handed.");
    } 

    state.turn += 1;
}

fn toggle_equipment(state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, gui: &mut GameUI) {
    let inv_items = game_objs.inv_slots_used();
    let slots: HashSet<char> = inv_items.iter().map(|i| i.0).collect();
    
    if slots.len() == 0 {
        state.write_msg_buff("You are empty handed.");
        return;
    }

    let sbi = state.curr_sidebar_info(player);
	if let Some(ch) = gui.query_single_response("Ready/unready what?", Some(&sbi)) {
        if !slots.contains(&ch) {
            state.write_msg_buff("You do not have that item!");
        } else {
            for i in inv_items {
                if let Some(item) = i.1.as_item() {
                    if item.slot == ch {
                        toggle_item(state, game_objs, item, player);
                        break;                       
                    }
                }
            }
        }
	} else {
        state.write_msg_buff("Nevermind.");
    }
}

fn use_item(state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, gui: &mut GameUI) {
    let inv_items = game_objs.inv_slots_used();
    let slots: HashSet<char> = inv_items.iter().map(|i| i.0).collect();
    
    if slots.len() == 0 {
        state.write_msg_buff("You are empty handed.");
        return;
    }
    
    let sbi = state.curr_sidebar_info(player);
    if let Some(ch) = gui.query_single_response("Use what?", Some(&sbi)) {
        if !slots.contains(&ch) {
            state.write_msg_buff("You do not have that item!");
            return;
        } 
        for i in inv_items {
            if let Some(mut item) = i.1.as_item() {
                if item.slot == ch {
                    if !item.useable() {
                        state.write_msg_buff("You don't know how to use that.");
                        return;
                    } 

                    let s = if item.active { 
                        format!("You extinguish {}.", item.name.with_def_article())
                    } else {
                        format!("{} blazes brightly!", item.name.with_def_article().capitalize())
                    };
                    state.write_msg_buff(&s);
                    
                    game_objs.get(item.get_object_id());
                    item.active = !item.active;
                    item.stackable = false;                    
                    if item.active {
                        game_objs.listeners.insert((item.get_object_id(), EventType::EndOfTurn));
                    } else {
                        game_objs.listeners.remove(&(item.get_object_id(), EventType::EndOfTurn));
                    }
                    game_objs.add_to_inventory(item);

                    state.turn += 1;
                                          
                    break;                       
                }
            }
        }
	} else {
        state.write_msg_buff("Nevermind.");
    }
}

fn get_move_tuple(mv: &str) -> (i32, i32) {
  	if mv == "N" {
		return (-1, 0);
	} else if mv == "S" {
		return (1, 0);
	} else if mv == "W" {
		return (0, -1);
	} else if mv == "E" {
		return (0, 1);
	} else if mv == "NW" {
		return (-1, -1);
	} else if mv == "NE" {
		return (-1, 1);
	} else if mv == "SW" {
		return (1, -1);
	} else {
		return (1, 1);
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
    state.turn += 1;
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
    state.turn += 1;
}

fn take_stairs(state: &mut GameState, player: &mut Player, down: bool) {
    let tile = &state.map[&player.location];

    if down {
        if *tile == map::Tile::Portal {
            state.write_msg_buff("You enter the beckoning portal.");
            player.location = (player.location.0, player.location.1, player.location.2 + 1);
            state.turn += 1;
        } else if *tile == map::Tile::StairsDown {
            state.write_msg_buff("You brave the stairs downward.");
            player.location = (player.location.0, player.location.1, player.location.2 + 1);
            state.turn += 1;
        } else {
            state.write_msg_buff("You cannot do that here.");
        }

        if player.location.2 > player.max_depth as i8 {
            player.max_depth = player.location.2 as u8;
        }
    } else {
        if *tile == map::Tile::StairsUp {
            state.write_msg_buff("You climb the stairway.");
            player.location = (player.location.0, player.location.1, player.location.2 - 1);
            state.turn += 1;
            
            if player.location.2 == 0 {
                state.write_msg_buff("Fresh air!");
            }
        } else {
            state.write_msg_buff("You cannot do that here.");
        }
    }
}

fn do_move(state: &mut GameState, player: &mut Player, game_objs: &GameObjects, dir: &str) {
	let mv = get_move_tuple(dir);

	let start_tile = &state.map[&player.location];
	let next_row = player.location.0 + mv.0;
	let next_col = player.location.1 + mv.1;
	let next_loc = (next_row, next_col, player.location.2);
	let tile = &state.map[&next_loc].clone();
	
    if game_objs.blocking_obj_at(&next_loc) {
        // Not quite ready to implement combat yet...
        state.write_msg_buff("There's someone in your way!");
    } else if tile.passable() {
		player.location = next_loc;

		match tile {
			map::Tile::Water => state.write_msg_buff("You splash in the shallow water."),
			map::Tile::DeepWater => {
				if *start_tile != map::Tile::DeepWater {
					state.write_msg_buff("You begin to swim.");				
				}
			},
			map::Tile::Lava => state.write_msg_buff("MOLTEN LAVA!"),
			map::Tile::FirePit => {
				state.write_msg_buff("You step in the fire!");
			},
			map::Tile::OldFirePit => state.write_msg_buff("An old campsite! Rum runners? A castaway?"),
            map::Tile::Portal => state.write_msg_buff("Where could this lead..."),
			_ => {
				if *start_tile == map::Tile::DeepWater { 
					state.write_msg_buff("Whew, you stumble ashore.");
				}
			},
		}

        let items = game_objs.descs_at_loc(next_loc);
        if items.len() == 1 {
            let s = format!("You see {} here.", items[0]);
            state.write_msg_buff(&s);
        } else if items.len() == 2 {
            let s = format!("You see {} and {} here.", items[0], items[1]);
            state.write_msg_buff(&s);
        } else if items.len() > 2 {
            state.write_msg_buff("There are several items here.");
        }
        
		state.turn += 1;
	} else  {
		state.write_msg_buff("You cannot go that way.");
	}
}

fn chat_with(state: &mut GameState, gui: &mut GameUI, loc: (i32, i32, i8), player: &mut Player, game_objs: &mut GameObjects, dialogue: &DialogueLibrary) {
    if let Some(mut npc) = game_objs.npc_at(&loc) {
        let line = npc.talk_to(state, player, dialogue);
        state.add_to_msg_history(&line);
        gui.popup_msg(&npc.get_fullname(), &line);
        game_objs.add(npc);
    } else {
        if let Tile::Door(_) = state.map[&loc] {
            state.write_msg_buff("The door is ignoring you.");
        } else {
            state.write_msg_buff("Oh no, talking to yourself?");
        } 
    }
}

fn show_character_sheet(gui: &mut GameUI, player: &Player) {
	let s = format!("{}, a {} level {}", player.name, util::num_to_nth(player.level), player.role.desc());
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
	let s = format!("AC: {}    Hit Points: {}({})", 10, player.curr_hp, player.max_hp);
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

fn show_inventory(gui: &mut GameUI, state: &mut GameState, player: &Player, game_objs: &GameObjects) {
    let menu = game_objs.get_inventory_menu();

    let money = if player.purse == 1 {
        String::from("$) a single zorkmid to your name")
    } else {
        let s = format!("$) {} gold pieces", player.purse);
        s
    };

	if menu.len() == 0 && player.purse == 0 {
		state.write_msg_buff("You are empty-handed.");
	} else {
		let mut m: Vec<&str> = menu.iter().map(AsRef::as_ref).collect();        
        m.insert(0, "You are carrying:");
        if player.purse > 0 {
            m.insert(1, &money);
        }
		gui.write_long_msg(&m, true);
	}
}

fn wiz_command(state: &mut GameState, gui: &mut GameUI, player: &mut Player) {
    let sbi = state.curr_sidebar_info(player);
    match gui.query_user(":", 20, Some(&sbi)) {
        Some(result) => {
            let pieces: Vec<&str> = result.trim().split('=').collect();

            if result == "loc" {
                println!("{:?}", player.location);
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
        },
        None => { },
    }
}

fn confirm_quit(state: &GameState, gui: &mut GameUI, player: &Player) -> Result<(), ExitReason> {
	let sbi = state.curr_sidebar_info(player);
	match gui.query_yes_no("Do you really want to Quit? (y/n)", Some(&sbi)) {
		'y' => Err(ExitReason::Quit),
		_ => Ok(()),
	}
}

fn pick_player_start_loc(state: &GameState) -> (i32, i32, i8) {
    let x = thread_rng().gen_range(0, 4);
    let b = state.world_info.town_boundary;

    return state.world_info.facts[0].location;

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
fn fov_to_tiles(state: &mut GameState, game_objs: &GameObjects, visible: &Vec<((i32, i32, i8), bool)>) -> Vec<(map::Tile, bool)> {
    let mut v_matrix = vec![(map::Tile::Blank, false); visible.len()];
    
    for j in 0..visible.len() {
        let vis = visible[j];
        if vis.0 == state.player_loc {
            v_matrix[j] = (map::Tile::Player(WHITE), true);
        } else if visible[j].1 {      
            let tile = if let Some(t) = game_objs.tile_at(&vis.0) {
                if !t.1 {
                    state.tile_memory.insert(vis.0, t.0);
                }
                t.0
            } else {
                state.tile_memory.insert(vis.0, state.map[&vis.0]);
                // I wanted to make tochlight squares be coloured different so this is a slight
                // kludge. Although perhaps later I might use it to differentiate between a player
                // walking through the dungeon with a light vs relying on darkvision, etc
                if state.lit_sqs.contains(&vis.0) && state.map[&vis.0] == Tile::StoneFloor {
                    Tile::ColourFloor(YELLOW)
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

fn run(gui: &mut GameUI, state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, dialogue: &DialogueLibrary) -> Result<(), ExitReason> {
    let visible = fov::calc_fov(state, player.location, player.vision_radius, FOV_HEIGHT, FOV_WIDTH);
	gui.v_matrix = fov_to_tiles(state, game_objs, &visible);
    let sbi = state.curr_sidebar_info(player);
    gui.write_screen(&mut state.msg_buff, Some(&sbi));

    loop {
        let start_turn = state.turn;
        let cmd = gui.get_command(&state, &player);
        match cmd {
            Cmd::Chat(loc) => chat_with(state, gui, loc, player, game_objs, dialogue),
            Cmd::Close(loc) => do_close(state, loc),
            Cmd::Down => take_stairs(state, player, true),
            Cmd::DropItem => drop_item(state, player, game_objs, gui),  
            Cmd::Move(dir) => do_move(state, player, game_objs, &dir),
            Cmd::MsgHistory => show_message_history(state, gui),
            Cmd::Open(loc) => do_open(state, loc),
            Cmd::Pass => {
                state.turn += 1;
                println!("{:?}", state.curr_time());
            },
            Cmd::PickUp => pick_up(state, player, game_objs, gui),
            Cmd::Save => save_and_exit(state, game_objs, player, gui)?,
            Cmd::ShowCharacterSheet => show_character_sheet(gui, player),
            Cmd::ShowInventory => show_inventory(gui, state, player, game_objs),
            Cmd::ToggleEquipment => toggle_equipment(state, player, game_objs, gui),
            Cmd::Use => use_item(state, player, game_objs, gui),
            Cmd::Quit => confirm_quit(state, gui, player)?,
            Cmd::Up => take_stairs(state, player, false),
            Cmd::WizardCommand => wiz_command(state, gui, player),
            _ => continue,
        }
        
        state.player_loc = player.location;

        if state.turn > start_turn {
            game_objs.do_npc_turns(state);
            game_objs.end_of_turn(state);
        }
        
        player.calc_vision_radius(state, game_objs);
        
        //let fov_start = Instant::now();
        let visible = fov::calc_fov(state, player.location, player.vision_radius, FOV_HEIGHT, FOV_WIDTH);
        gui.v_matrix = fov_to_tiles(state, game_objs, &visible);        
        //let fov_duration = fov_start.elapsed();
        //println!("Time for fov: {:?}", fov_duration);
		
        //let write_screen_start = Instant::now();
        let sbi = state.curr_sidebar_info(player);
        gui.write_screen(&mut state.msg_buff, Some(&sbi));
        //let write_screen_duration = write_screen_start.elapsed();
        //println!("Time for write_screen(): {:?}", write_screen_duration);        
    }
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


    let dialogue_library = dialogue::read_dialogue_lib();
    let player_name = who_are_you(&mut gui);
    
    let mut game_objs: GameObjects;
    let mut state: GameState;
    let mut player: Player;
    if existing_save_file(&player_name) {
        if let Some(saved_objs) = fetch_saved_data(&player_name) {
            state = saved_objs.0;
            game_objs = saved_objs.1;
            player = saved_objs.2;

            let msg = format!("Welcome back, {}!", player.name);
            state.write_msg_buff(&msg);
        } else {
            // need to dump some sort of message for corrupted game file
            return;
        }
    } else {
        game_objs = GameObjects::new();                
        let w = world::generate_world(&mut game_objs);        
        state = GameState::init(w.0, w.1);    

        player = fetch_player(&state, &mut game_objs, &mut gui, player_name).unwrap();
        state.player_loc = player.location;
    
        //let sbi = state.curr_sidebar_info(&player);
        //gui.write_screen(&mut state.msg_buff, Some(&sbi));
        state.write_msg_buff("Welcome, adventurer!");   
	
    }
    
    match run(&mut gui, &mut state, &mut player, &mut game_objs, &dialogue_library) {
		Ok(_) => println!("Game over I guess? Probably the player won?!"),
		//Err(ExitReason::Save) => save_msg(&mut state, &mut gui),
		//Err(ExitReason::Quit) => quit_msg(&mut state, &mut gui),
		//Err(ExitReason::Win) => victory_msg(&mut state, &mut gui),
		//Err(ExitReason::Death(src)) => death(&mut state, src, &mut gui),
        Err(_) => println!("okay bye"),
	}
}
