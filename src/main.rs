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
mod items;
mod map;
mod pathfinding;
mod player;
mod town;
mod util;
mod wilderness;
mod world;

use std::collections::{HashMap, VecDeque, HashSet};
use std::path::Path;
//use std::time::{Duration, Instant};

use rand::{Rng, thread_rng};

use actor::Actor;
use dialogue::DialogueLibrary;
use display::{GameUI, SidebarInfo, WHITE};
use items::{Item, ItemPile};
use map::{Tile, DoorState};
use player::Player;
use world::WorldInfo;

const MSG_HISTORY_LENGTH: usize = 50;
const FOV_WIDTH: usize = 41;
const FOV_HEIGHT: usize = 21;

pub type Items = HashMap<(i32, i32, i8), ItemPile>;
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
    seen_sqs: HashSet<(i32, i32, i8)>,
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
            seen_sqs: HashSet::new(),
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

fn start_new_game(state: &GameState, gui: &mut GameUI, player_name: String) -> Option<Player> {
    let mut menu = vec!["Welcome adventurer, please choose your role in RogueVillage:"];
	menu.push("");
	menu.push("  (a) Human Warrior - a doughty fighter who lives by the sword and...well");
    menu.push("                      hopefully just that first part.");
	menu.push("");
	menu.push("  (b) Human Rogue - a quick, sly adventurer who gets by on their light step");
    menu.push("                    and fast blade.");
	
    if let Some(answer) = gui.menu_picker(&menu, 2, true, true) {
        if answer.contains(&0) {
            let mut player = Player::new_warrior(player_name);
            player.location = pick_player_start_loc(&state);
            return Some(player);
        } else {
            let mut player = Player::new_rogue(player_name);
            player.location = pick_player_start_loc(&state);
            return Some(player);
        }
    }

    None
}

fn fetch_player(state: &GameState, gui: &mut GameUI, player_name: String) -> Option<Player> {
    // Of course eventually this is where I'll check for a saved game and load
    // it if one exists.
    start_new_game(state, gui, player_name)
}

fn item_hits_ground(loc: (i32, i32, i8), item: Item, items: &mut Items) {
    if !items.contains_key(&loc) {
        items.insert(loc, ItemPile::new());
    }

    let mut item_copy = item.clone();
    item_copy.equiped = false;
    items.get_mut(&loc).unwrap().add(item_copy);
}

fn drop_zorkmids(loc: (i32, i32, i8), amt: u32, items: &mut Items) {
    for _ in 0..amt {
        item_hits_ground(loc, Item::get_item("gold piece").unwrap(), items)
    }
}

fn drop_item(state: &mut GameState, player: &mut Player, items: &mut Items, gui: &mut GameUI) {
	if player.inventory.get_menu().len() == 0 {
		state.write_msg_buff("You are empty handed.");
		return
	}

	let sbi = state.curr_sidebar_info(player);
	if let Some(ch) = gui.query_single_response("Drop what?", Some(&sbi)) {
        if ch == '$' {
            let amt = gui.query_natural_num("How much?", Some(&sbi)).unwrap();
            if amt == 0 {
                state.write_msg_buff("Never mind.");                
            } else {
                if amt >= player.inventory.purse {
                    state.write_msg_buff("You drop all your money.");
                    drop_zorkmids(player.location, player.inventory.purse, items);
                    player.inventory.purse = 0;
                } else if amt > 1 {
                    let s = format!("You drop {} gold pieces.", amt);
                    state.write_msg_buff(&s);
                    drop_zorkmids(player.location, amt, items);
                    player.inventory.purse -= amt;
                } else {
                    state.write_msg_buff("You drop a gold piece.");
                    drop_zorkmids(player.location, 1, items);
                    player.inventory.purse -= 1;
                }
                state.turn += 1;
            }
        } else {
            let count = player.inventory.count_in_slot(ch);
            if count == 0 {
                state.write_msg_buff("You do not have that item.");
            } else if count > 1 {
                match gui.query_natural_num("Drop how many?", Some(&sbi)) {
                    Some(v) => {
                        let pile = player.inventory.remove_count(ch, v);
                        if pile.len() > 0 {
                            if v == 1 {
                                let s = format!("You drop the {}.", pile[0].name);
                                state.write_msg_buff(&s);
                            } else {
                                let pluralized = util::pluralize(&pile[0].name);
                                let s = format!("You drop {} {}.", v, pluralized);
                                state.write_msg_buff(&s);
                            }
                            state.turn += 1;
                            for item in pile {
                                item_hits_ground(player.location, item, items);
                            }
                        } else {
                            state.write_msg_buff("Nevermind.");
                        }
                    },
                    None => state.write_msg_buff("Nevermind."),
                }
            } else {
                let mut item = player.inventory.remove(ch);
                item.equiped = false;
                let s = format!("You drop the {}.", util::get_articled_name(true, &item));                
                item_hits_ground(player.location, item, items);
                state.write_msg_buff(&s);
                state.turn += 1;
            }	
        }		
	} else {
        state.write_msg_buff("Nevermind.")
    }

    //state.player.calc_ac();
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

fn do_move(state: &mut GameState, player: &mut Player, npcs: &NPCTable, items: &Items, dir: &str) {
	let mv = get_move_tuple(dir);

	let start_tile = &state.map[&player.location];
	let next_row = player.location.0 + mv.0;
	let next_col = player.location.1 + mv.1;
	let next_loc = (next_row, next_col, player.location.2);
	let tile = &state.map[&next_loc].clone();
	
    if npcs.contains_key(&next_loc) {
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

        if items.contains_key(&next_loc) {
            if items[&next_loc].pile.len() == 1 {
                let s = format!("You see {} here.", items[&next_loc].get_item_name(0));
                state.write_msg_buff(&s);
            } else if items[&next_loc].pile.len() == 2 {
                let s = format!("You see {} and {} here.", items[&next_loc].get_item_name(0), items[&next_loc].get_item_name(1));
                state.write_msg_buff(&s);
            } else {
                state.write_msg_buff("There are several items here.");
            }
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

fn show_inventory(gui: &mut GameUI, state: &mut GameState, player: &Player) {
    let menu = player.inventory.get_menu();

	if menu.len() == 0 {
		state.write_msg_buff("You are empty-handed.");
	} else {
		let mut m: Vec<&str> = menu.iter().map(AsRef::as_ref).collect();
        m.insert(0, "You are carrying:");
		gui.write_long_msg(&m, true);
	}
}

fn wiz_command(state: &mut GameState, gui: &mut GameUI, player: &mut Player) {
    let sbi = state.curr_sidebar_info(player);
    match gui.query_user(":", 20, Some(&sbi)) {
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
//
// This will soon be complicated by having items on the ground and perhaps eventually by independent light sources.
// Come to think of it, the memory of squares seen should be a memory of the last tile seen on it. Otherwise, if 
// a monster picks up an item the player has left on the ground while the player isn't around, the item will disappear
// from view even when the square hasn't subsequently been seen by the player. But the tile memory should only contain
// items or the ground square. I'll worry about that when I implement items.
fn fov_to_tiles(state: &GameState, npcs: &NPCTable, items: &Items, visible: &Vec<((i32, i32, i8), bool)>) -> Vec<(map::Tile, bool)> {
    let mut v_matrix = vec![(map::Tile::Blank, false); visible.len()];

    for j in 0..visible.len() {
        let vis = visible[j];
        if vis.0 == state.player_loc {
            v_matrix[j] = (map::Tile::Player(WHITE), true);
        } else if visible[j].1 {            
            if npcs.contains_key(&vis.0) {
                v_matrix[j] = (npcs[&vis.0].get_tile(), true);
            } else if items.contains_key(&vis.0) {
                v_matrix[j] = (items[&vis.0].get_tile(), true);
            } else {
                v_matrix[j] = (state.map[&vis.0], true);
            }
        } else if state.seen_sqs.contains(&vis.0) {
            v_matrix[j] = (state.map[&vis.0], false);            
        }
    }

    v_matrix
}

fn run(gui: &mut GameUI, state: &mut GameState, player: &mut Player, npcs: &mut NPCTable, items: &mut Items, dialogue: &DialogueLibrary) {
    let visible = fov::calc_fov(state, player, FOV_HEIGHT, FOV_WIDTH);
	gui.v_matrix = fov_to_tiles(state, npcs, items, &visible);
    let sbi = state.curr_sidebar_info(player);
    state.write_msg_buff("Welcome, adventurer!");   
	gui.write_screen(&mut state.msg_buff, Some(&sbi));

    loop {
        let start_turn = state.turn;
        let cmd = gui.get_command(&state, &player);
        match cmd {
            Cmd::Chat(loc) => chat_with(state, gui, loc, player, npcs, dialogue),
            Cmd::Close(loc) => do_close(state, loc),
            Cmd::Down => take_stairs(state, player, true),
            Cmd::DropItem => drop_item(state, player, items, gui),  
            Cmd::Move(dir) => do_move(state, player, npcs, items, &dir),
            Cmd::MsgHistory => show_message_history(state, gui),
            Cmd::Open(loc) => do_open(state, loc),
            Cmd::Pass => {
                state.turn += 1;
                println!("{:?}", state.curr_time());
            },
            Cmd::ShowCharacterSheet => show_character_sheet(gui, player),
            Cmd::ShowInventory => show_inventory(gui, state, player),
            Cmd::Quit => break,        
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
        let visible = fov::calc_fov(state, player, FOV_HEIGHT, FOV_WIDTH);
        gui.v_matrix = fov_to_tiles(state, npcs, items, &visible);
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

    let dialogue_library = dialogue::read_dialogue_lib();
    
    let w = world::generate_world();
    let mut npcs = w.2;
    let mut items = w.3;

    let mut state = GameState::init(w.0, w.1);    
	
    title_screen(&mut gui);
    let player_name = who_are_you(&mut gui);

    let mut player = fetch_player(&state, &mut gui, player_name).unwrap();
    state.player_loc = player.location;

    let sbi = state.curr_sidebar_info(&player);
    gui.write_screen(&mut state.msg_buff, Some(&sbi));
    
    run(&mut gui, &mut state, &mut player, &mut npcs, &mut items, &dialogue_library);
}
