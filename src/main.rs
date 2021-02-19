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

mod actor;  
mod display;
mod dungeon;
mod fov;
mod map;
mod util;
mod wilderness;
mod world;

use crate::display::{GameUI, SidebarInfo};

use std::collections::{VecDeque, HashMap};
//use std::io::prelude::*;
//use std::fs;
//use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

use actor::Player;
use rand::Rng;

const MSG_HISTORY_LENGTH: usize = 50;
const FOV_WIDTH: usize = 41;
const FOV_HEIGHT: usize = 21;

pub type Map = HashMap<(i32, i32, i8), map::Tile>;

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
	Open,
    Close,
	Quaff,
	FireGun,
	Reload,
	Search,
	Read,
	Eat,
	Save,
    Chat,
    Use,
	Help,
    Down,
    Up,
}

pub struct GameState {
	msg_buff: VecDeque<String>,
	msg_history: VecDeque<(String, u32)>,
	map: Map,
    turn: u32,
    vision_radius: u8,
    player_loc: (i32, i32, i8),
}

impl GameState {
    pub fn init() -> GameState {
        let state = GameState {
            msg_buff: VecDeque::new(),
            msg_history: VecDeque::new(),
			map: HashMap::new(),
            turn: 0,
            vision_radius: 30,
            player_loc: (127, 127, 0),
        };

        state
    }

	pub fn write_msg_buff(&mut self, msg: &str) {
		let s = String::from(msg);
		self.msg_buff.push_back(s);

		if msg.len() > 0 {
			if self.msg_history.len() == 0 || msg != self.msg_history[0].0 {
				self.msg_history.push_front((String::from(msg), 1));
			} else {
				self.msg_history[0].1 += 1;
			}

			if self.msg_history.len() > MSG_HISTORY_LENGTH {
				self.msg_history.pop_back();
			}
		}
	}

    pub fn curr_sidebar_info(&self, player: &Player) -> SidebarInfo {
		SidebarInfo::new(player.name.to_string(), player.curr_hp, player.max_hp, self.turn)
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

	gui.write_long_msg(&lines, true);
}

fn title_screen(gui: &mut GameUI) {
	let mut lines = vec!["Welcome to Rogue Village 0.0.1!".to_string(), "".to_string()];
	lines.push("".to_string());
	lines.push("".to_string());
    lines.push("".to_string());
    lines.push("".to_string());
    lines.push("".to_string());
    lines.push("".to_string());
    lines.push("".to_string());
    lines.push("".to_string());
    lines.push("".to_string());
    lines.push("".to_string());
	lines.push("".to_string());
	lines.push("".to_string());
	lines.push("".to_string());
	lines.push("".to_string());
	lines.push("Rogue Village is copyright 2021 by Dana Larose, see COPYING for licence info.".to_string());
	
	gui.write_long_msg(&lines, true);
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

fn adjacent_door(state: &mut GameState, closed: bool) -> Option<(i32, i32, i8)> {
    let mut doors = 0;
    let mut door: (i32, i32, i8) = (0, 0, 0);
    for r in -1..2 {
        for c in -1..2 {
            if r == 0 && c == 0 {
                continue;
            }

            let dr = state.player_loc.0 as i32 + r;
            let dc = state.player_loc.1 as i32 + c;
            let loc = (dr, dc, state.player_loc.2);
            match &state.map[&loc] {
                map::Tile::Door(open) => {
                    if *open == closed {
                        doors += 1;
                        door = loc;
                    }
                },
                _ => { }
            }
        }
    }

    if doors == 1 {
        Some(door)
    } else {
        None
    }
}

fn do_open(state: &mut GameState, gui: &mut GameUI, player: &Player) {
    let mut door = (0, 0, 0);
    if let Some(d) = adjacent_door(state, false) {
        door = d;
    } else {
        match gui.pick_direction("Open what?", &state.curr_sidebar_info(player)) {
            Some(dir) => {
                let obj_row =  state.player_loc.0 as i32 + dir.0;
                let obj_col = state.player_loc.1 as i32 + dir.1;
                let loc = (obj_row, obj_col, state.player_loc.2);
                let tile = &state.map[&loc];
                match tile {
                    map::Tile::Door(true) => state.write_msg_buff("The door is already open!"),
                    map::Tile::Door(false) => door = loc,
                    _ => state.write_msg_buff("You cannot open that!"),
                }
                state.turn += 1;
            },
            None => state.write_msg_buff("Nevermind."),
        }
    }
    
    if door != (0, 0, 0) {
        state.write_msg_buff("You open the door!");
        state.map.insert(door, map::Tile::Door(true));    
    }    
}

fn do_close(state: &mut GameState, gui: &mut GameUI, player: &Player) {
    let mut door = (0, 0, 0);
    if let Some(d) = adjacent_door(state, true) {
        door = d;
    } else {
        match gui.pick_direction("Close what?", &state.curr_sidebar_info(player)) {
            Some(dir) => {
                let obj_row =  state.player_loc.0 as i32 + dir.0;
                let obj_col = state.player_loc.1 as i32 + dir.1;
                let loc = (obj_row, obj_col, state.player_loc.2);
                let tile = &state.map[&loc];
                match tile {
                    map::Tile::Door(false) => state.write_msg_buff("The door is already closed!"),
                    map::Tile::Door(true) => door = loc,
                    _ => state.write_msg_buff("You cannot close that!")
                }
                state.turn += 1;
            },
            None => state.write_msg_buff("Nevermind."),
        }
    }

    if door != (0, 0, 0) {
        state.write_msg_buff("You close the door!");
        state.map.insert(door, map::Tile::Door(false));
    }
}

fn take_stairs(state: &mut GameState, gui: &mut GameUI, down: bool) {
    let tile = &state.map[&state.player_loc];

    if down {
        if *tile == map::Tile::Portal {
            state.write_msg_buff("You enter the beckoning portal.");
            state.player_loc = (state.player_loc.0, state.player_loc.1, state.player_loc.2 + 1);
            state.turn += 1;
        } else if *tile == map::Tile::StairsDown {
            state.write_msg_buff("You brave the stairs downward.");
            state.player_loc = (state.player_loc.0, state.player_loc.1, state.player_loc.2 + 1);
            state.turn += 1;
        } else {
            state.write_msg_buff("You cannot do that here.");
        }
    } else {
        if *tile == map::Tile::StairsUp {
            state.write_msg_buff("You climb the stairway.");
            state.player_loc = (state.player_loc.0, state.player_loc.1, state.player_loc.2 - 1);
            state.turn += 1;
            
            if state.player_loc.2 == 0 {
                state.write_msg_buff("Fresh air!");
            }
        } else {
            state.write_msg_buff("You cannot do that here.");
        }
    }
}

fn do_move(state: &mut GameState, dir: &str, gui: &mut GameUI) {
	let mv = get_move_tuple(dir);

	let start_tile = &state.map[&state.player_loc];
	let next_row = state.player_loc.0 + mv.0;
	let next_col = state.player_loc.1 + mv.1;
	let next_loc = (next_row, next_col, state.player_loc.2);
	let tile = &state.map[&next_loc].clone();
	
	if tile.is_passable() {
		state.player_loc = next_loc;

		match tile {
			map::Tile::Water => state.write_msg_buff("You splash in the shallow water."),
			map::Tile::DeepWater => {
				if *start_tile != map::Tile::DeepWater {
					state.write_msg_buff("You begin to swim.");				
				}

				//if state.player.curr_stamina < 10 {
				//	state.write_msg_buff("You're getting tired...");
				//}
			},
			map::Tile::Lava => state.write_msg_buff("MOLTEN LAVA!"),
			map::Tile::FirePit => {
				state.write_msg_buff("You step in the fire!");
			},
			map::Tile::OldFirePit => state.write_msg_buff("An old campsite! Rum runners? A castaway?"),
            map::Tile::Portal => state.write_msg_buff("Where could this lead..."),
			_ => {
				if *start_tile == map::Tile::DeepWater { // && state.player.curr_stamina < 10 {
					state.write_msg_buff("Whew, you stumble ashore.");
				}
			},
		}

		state.turn += 1;
	} else  {
		state.write_msg_buff("You cannot go that way.");
	}
}

fn run(gui: &mut GameUI, state: &mut GameState, player: &mut Player) {
    state.write_msg_buff("Hello, world?");

	gui.v_matrix = fov::calc_v_matrix(state, FOV_HEIGHT, FOV_WIDTH);
    let sbi = state.curr_sidebar_info(player);
	gui.write_screen(&mut state.msg_buff, &sbi);

    loop {
        let size = FOV_HEIGHT * FOV_WIDTH;

        let start_turn = state.turn;
        let cmd = gui.get_command(&state);
        match cmd {
            // Cmd::Chat => {
            //     gui.popup_msg("Dale, the Innkeeper", "Welcome to Skara Brae, stranger! You'll find the dungeon in the foothills but watch out for goblins on the way!");
            // },
            Cmd::Pass => state.turn += 1,
            Cmd::Quit => break,
            Cmd::MsgHistory => show_message_history(state, gui),
			Cmd::Move(dir) => do_move(state, &dir, gui),
            Cmd::Open => do_open(state, gui, player),
            Cmd::Close => do_close(state, gui, player),            
            Cmd::Down => take_stairs(state, gui, true),
            Cmd::Up => take_stairs(state, gui, false),
            _ => continue,
        }
        
        //let fov_start = Instant::now();
        player.calc_vision_radius(state);
        gui.v_matrix = fov::calc_v_matrix(state, FOV_HEIGHT, FOV_WIDTH);
        //let fov_duration = fov_start.elapsed();
        //println!("Time for fov: {:?}", fov_duration);
		
        //let write_screen_start = Instant::now();
        let sbi = state.curr_sidebar_info(player);
        gui.write_screen(&mut state.msg_buff, &sbi);
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

    let mut state = GameState::init();
	state.map = world::generate_world();
    world::find_lost_valleys(&state.map, 257);

    title_screen(&mut gui);

    let mut player = Player::new(String::from("Dana"));

    let sbi = state.curr_sidebar_info(&player);
    gui.write_screen(&mut state.msg_buff, &sbi);
    
    run(&mut gui, &mut state, &mut player);
}
