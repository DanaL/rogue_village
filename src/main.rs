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
mod dialogue;
mod display;
mod dungeon;
mod fov;
mod map;
mod pathfinding;
mod town;
mod util;
mod wilderness;
mod world;



use std::collections::{VecDeque, HashMap};
use std::path::Path;
//use std::time::{Duration, Instant};

use rand::{Rng, thread_rng};

use actor::Actor;
use actor::Player;
use dialogue::DialogueLibrary;
use display::{GameUI, SidebarInfo, WHITE};
use map::{Tile, DoorState};
use world::WorldInfo;

const MSG_HISTORY_LENGTH: usize = 50;
const FOV_WIDTH: usize = 41;
const FOV_HEIGHT: usize = 21;

pub type Map = HashMap<(i32, i32, i8), map::Tile>;
pub type NPCTable = HashMap<(i32, i32, i8), Box<dyn Actor>>;

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

pub struct GameState {
	msg_buff: VecDeque<String>,
	msg_history: VecDeque<(String, u32)>,
	map: Map,
    turn: u32,
    vision_radius: u8,
    player_loc: (i32, i32, i8),
    world_info: WorldInfo,
}

impl GameState {
    pub fn init(map: Map, world_info: WorldInfo) -> GameState {
        let state = GameState {
            msg_buff: VecDeque::new(),
            msg_history: VecDeque::new(),
			map: map,
            turn: 0,
            vision_radius: 30,
            player_loc: (-1, -1, -1),
            world_info: world_info,
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
		SidebarInfo::new(player.name.to_string(), player.curr_hp, player.max_hp, self.turn)
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

fn do_move(state: &mut GameState, player: &mut Player, npcs: &NPCTable, dir: &str) {
	let mv = get_move_tuple(dir);

	let start_tile = &state.map[&player.location];
	let next_row = player.location.0 + mv.0;
	let next_col = player.location.1 + mv.1;
	let next_loc = (next_row, next_col, player.location.2);
	let tile = &state.map[&next_loc].clone();
	
    if npcs.contains_key(&next_loc) {
        // Not quite ready to implement combat yet...
        state.write_msg_buff("There's someone in your way!");
    } else if tile.is_passable() {
		player.location = next_loc;

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

fn chat_with(state: &mut GameState, gui: &mut GameUI, loc: (i32, i32, i8), player: &mut Player, npcs: &mut NPCTable, dialogue: &DialogueLibrary) {
    if !npcs.contains_key(&loc) {
        if let Tile::Door(_) = state.map[&loc] {
            state.write_msg_buff("The door is ignoring you.");
        } else {
            state.write_msg_buff("Oh no, talking to yourself?");
        } 
    } else {
        let mut npc = npcs.remove(&loc).unwrap();
        let line = npc.talk_to(state, player, dialogue);
        state.add_to_msg_history(&line);
        gui.popup_msg(&npc.get_name(), &line);
        npcs.insert(loc, npc);
        
        state.turn += 1;
    }    
}

fn wiz_command(state: &mut GameState, gui: &mut GameUI, player: &mut Player) {
    let sbi = state.curr_sidebar_info(player);
    match gui.query_user(":", 20, &sbi) {
        Some(result) => {
            let pieces: Vec<&str> = result.trim().split('=').collect();
            if pieces.len() != 2 {
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

fn pick_player_start_loc(state: &GameState) -> (i32, i32, i8) {
    let x = thread_rng().gen_range(0, 4);
    let b = state.world_info.town_boundary;

    if x == 0 {
        (b.0 - 5, thread_rng().gen_range(b.1, b.3), 0)
    } else if x == 1 {
        (b.2 + 5, thread_rng().gen_range(b.1, b.3), 0)
    } else if x == 2 {
        (thread_rng().gen_range(b.0, b.2), b.1 - 5, 0)
    } else {
        (thread_rng().gen_range(b.0, b.2), b.3 + 5, 0)
    }
}

// Top tiles as in which tile is sitting on top of the square. NPC, items (eventually, once I 
// implement them), weather (ditto), etc and at the bottom, the terrain tile
fn get_top_tiles(map: &Map, player: &Player, npcs: &NPCTable) -> Map {
    let mut tiles = HashMap::new();
    let half_fov_h = FOV_HEIGHT as i32 / 2;
    let half_fov_w = FOV_WIDTH as i32 / 2;
    
    for r in player.location.0 - half_fov_h..player.location.0 + half_fov_h{
        for c in player.location.1 - half_fov_w..player.location.1 + half_fov_w {
            let loc = (r, c, player.location.2);
            if npcs.contains_key(&loc) {
                tiles.insert(loc, npcs[&loc].get_tile());
            } else if map.contains_key(&loc) {
                tiles.insert(loc, map[&loc]);
            }
        }
    }

    tiles.insert(player.location, map::Tile::Player(WHITE));

    tiles
}

fn run(gui: &mut GameUI, state: &mut GameState, player: &mut Player, npcs: &mut NPCTable, dialogue: &DialogueLibrary) {
    let tiles = get_top_tiles(&state.map, player, npcs);
	gui.v_matrix = fov::calc_v_matrix(&tiles, player, FOV_HEIGHT, FOV_WIDTH);
    let sbi = state.curr_sidebar_info(player);
	gui.write_screen(&mut state.msg_buff, &sbi);

    loop {
        let start_turn = state.turn;
        let cmd = gui.get_command(&state, &player);
        match cmd {
            Cmd::Chat(loc) => chat_with(state, gui, loc, player, npcs, dialogue),
            Cmd::Pass => {
                state.turn += 1;
                println!("{:?}", state.curr_time());
            },
            Cmd::Quit => break,
            Cmd::MsgHistory => show_message_history(state, gui),
			Cmd::Move(dir) => do_move(state, player, npcs, &dir),
            Cmd::Open(loc) => do_open(state, loc),
            Cmd::Close(loc) => do_close(state, loc),            
            Cmd::Down => take_stairs(state, player, true),
            Cmd::Up => take_stairs(state, player, false),
            Cmd::WizardCommand => wiz_command(state, gui, player),
            _ => continue,
        }
        
        state.player_loc = player.location;

        if state.turn > start_turn {
            let npc_locs = npcs.keys()
						.map(|k| k.clone())
						.collect::<Vec<(i32, i32, i8)>>();
            
            for loc in npc_locs {
                // remove the npc from the table so that we can pass a reference
                // to the NPCTable to its act() function
                let mut npc = npcs.remove(&loc).unwrap();

                npc.act(state, npcs);
                let curr_loc = npc.get_loc();

                // after it's done its turn, re-insert it back into the table
                npcs.insert(curr_loc, npc);
            }            
        }

        player.calc_vision_radius(state);
        
        //let fov_start = Instant::now();
        let tiles = get_top_tiles(&state.map, player, npcs);
        gui.v_matrix = fov::calc_v_matrix(&tiles, player, FOV_HEIGHT, FOV_WIDTH);
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

    let dialogue_library = dialogue::read_dialogue_lib();
    
    let w = world::generate_world();
    let mut npcs = w.2;

    let mut state = GameState::init(w.0, w.1);    
	
    title_screen(&mut gui);

    let mut player = Player::new(String::from("Dana"));
    player.location = pick_player_start_loc(&state);
    state.player_loc = player.location;

    let sbi = state.curr_sidebar_info(&player);
    gui.write_screen(&mut state.msg_buff, &sbi);
    
    run(&mut gui, &mut state, &mut player, &mut npcs, &dialogue_library);
}
