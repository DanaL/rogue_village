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
use std::path::Path;
use std::time::{Duration, Instant};

use rand::{Rng, thread_rng};

use dialogue::DialogueLibrary;
use display::{GameUI, SidebarInfo, WHITE, YELLOW};
use game_obj::{GameObject, GameObjects};
use items::{GoldPile, Item, ItemPile};
use map::{Tile, DoorState};
use player::Player;
use util::StringUtils;
use world::WorldInfo;

const MSG_HISTORY_LENGTH: usize = 50;
const FOV_WIDTH: usize = 41;
const FOV_HEIGHT: usize = 21;
const PLAYER_INV: (i32, i32, i8) = (-999, -999, -128);

pub type Map = HashMap<(i32, i32, i8), map::Tile>;

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum EventType {
    EndOfTurn,
    LightExpired,
    TakeTurn,
}

pub trait EventListener {
    fn receive(&mut self, event: EventType, state: &mut GameState) -> Option<EventType>;
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

pub struct GameState {
	msg_buff: VecDeque<String>,
	msg_history: VecDeque<(String, u32)>,
	map: Map,
    turn: u32,
    player_loc: (i32, i32, i8),
    world_info: WorldInfo,
    tile_memory: HashMap<(i32, i32, i8), Tile>,
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
        let weapon = if let Some(w) = player.inventory.get_readied_weapon() {
            w.name.capitalize()    
        } else {
            String::from("Empty handed")
        };

		SidebarInfo::new(player.name.to_string(), player.curr_hp, player.max_hp, self.turn, player.ac,
         player.inventory.purse, weapon)
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

fn item_hits_ground(loc: (i32, i32, i8), item: Item, game_objs: &mut GameObjects) {
    // if !items.contains_key(&loc) {
    //     items.insert(loc, ItemPile::new());
    // }

    // println!("foo {}", item.object_id);

    // let mut item_copy = item.clone();
    // item_copy.equiped = false;
    // items.get_mut(&loc).unwrap().add(item_copy);
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
            let zorkmids = GoldPile::new(game_objs.next_id(), amt, player.location);
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

fn drop_item(state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, gui: &mut GameUI) {    
	// if player.inventory.get_menu().len() == 0 {
	// 	state.write_msg_buff("You are empty handed.");
	// 	return
	// }

    let sbi = state.curr_sidebar_info(player);
    if let Some(ch) = gui.query_single_response("Drop what?", Some(&sbi)) {
        if ch == '$' {
            drop_zorkmids(state, player, game_objs, gui);
        }
    }
    /* 
	let sbi = state.curr_sidebar_info(player);
	if let Some(ch) = gui.query_single_response("Drop what?", Some(&sbi)) {
        if ch == '$' {
            
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
                                let s = format!("You drop {} {}.", v, &pile[0].name.pluralize());
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
                let s = format!("You drop {}.", &item.name.with_def_article());                
                item_hits_ground(player.location, item, items);
                state.write_msg_buff(&s);
                state.turn += 1;
            }	
        }		
	} else {
        state.write_msg_buff("Nevermind.")
    }

    player.calc_ac();
    */
}

fn pick_up_item_or_stack(state: &mut GameState, player: &mut Player, item: (Item, u16)) {
    /*
    if item.1 == 1 {
		let s = format!("You pick up {}.", &item.0.name.with_def_article());
		state.write_msg_buff(&s);
		player.inventory.add(item.0);
    } else {
        let s = format!("You pick up {} {}.", item.1, &item.0.name.pluralize());
		state.write_msg_buff(&s);

        for _ in 0..item.1 {
            player.inventory.add(item.0.clone());
        }
    }
    */
}

fn pick_up(state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, gui: &mut GameUI) {
    /*
	if !items.contains_key(&player.location) {
		state.write_msg_buff("There is nothing here to pick up.");
        return;
	} 
    
    let item_count = items[&player.location].pile.len();	
    if item_count == 1 {
		let item = items.get_mut(&player.location).unwrap().get();
        pick_up_item_or_stack(state, player, item);
        items.remove(&player.location);
		state.turn += 1;
	} else {
		let mut m = items[&player.location].get_menu();
		m.insert(0, "Pick up what: (* to get everything)".to_string());
        let menu = m.iter().map(AsRef::as_ref).collect();
		let answers = gui.menu_picker(&menu, menu.len() as u8, false, false);
		match answers {
			None => state.write_msg_buff("Nevermind."), // Esc was pressed
			Some(v) => {
				state.turn += 1;
				let picked_up = items.get_mut(&player.location).unwrap().get_many(&v);
				for item in picked_up {
                    pick_up_item_or_stack(state, player, item);                    
				}
                
                if items[&player.location].pile.len() == 0 {
                    items.remove(&player.location);
                }
			},
		}
	}
    */
}

fn toggle_equipment(state: &mut GameState, player: &mut Player, gui: &mut GameUI) {
    /*
    if player.inventory.used_slots().len() == 0 {
		state.write_msg_buff("You are empty handed.");
		return
	}

	let sbi = state.curr_sidebar_info(player);
	if let Some(ch) = gui.query_single_response("Ready/unready what?", Some(&sbi)) {
        let result = player.inventory.toggle_slot(ch);
        state.write_msg_buff(&result.0);
		state.turn += 1;		
	} else {
        state.write_msg_buff("Nevermind.");
    }

	player.calc_ac();
    */
}

fn use_item(state: &mut GameState, player: &mut Player, gui: &mut GameUI) {
    /*
    if player.inventory.used_slots().len() == 0 {
		state.write_msg_buff("You are empty handed.");
		return
	}

    let sbi = state.curr_sidebar_info(player);
    if let Some(ch) = gui.query_single_response("Use what?", Some(&sbi)) {
        if let Some(item) = player.inventory.peek_at(ch) {
            if !item.useable() {
                state.write_msg_buff("You don't know how to use that.");
            } else {
                // I suspect this will get much more complicated when there are more types of items but 
                // for now it's really just torches.
                let msg = player.inventory.use_item_in_slot(ch, state);
                state.write_msg_buff(&msg);
            }
        } else {
            state.write_msg_buff("You do not have that item.");
        }        
	} else {
        state.write_msg_buff("Nevermind.");
    }
    */
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
        m.insert(1, &money);
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
fn fov_to_tiles(state: &mut GameState, player: &Player, game_objs: &GameObjects, visible: &Vec<((i32, i32, i8), bool)>) -> Vec<(map::Tile, bool)> {
    let mut v_matrix = vec![(map::Tile::Blank, false); visible.len()];
    let underground = state.player_loc.2 > 0;
    let has_light = player.inventory.light_from_items() > 0;

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
                state.map[&vis.0]
            };
            
            // I wanted to make tochlight squares be coloured different so this is a slight
            // kludge. Although perhaps later I might use it to differentiate between a player
            // walking through the dungeon with a light vs relying on darkvision, etc
            // if underground && has_light && state.map[&vis.0] == Tile::StoneFloor {
            //     Tile::ColourFloor(YELLOW)
            // } else {
            //     state.map[&vis.0]
            // }
                        
            v_matrix[j] = (tile, true);
        } else if state.tile_memory.contains_key(&vis.0) {
            v_matrix[j] = (state.tile_memory[&vis.0], false);            
        }
    }

    v_matrix
}

fn run(gui: &mut GameUI, state: &mut GameState, player: &mut Player, game_objs: &mut GameObjects, dialogue: &DialogueLibrary) {
    let visible = fov::calc_fov(&state.map, player, FOV_HEIGHT, FOV_WIDTH);
	gui.v_matrix = fov_to_tiles(state, player, game_objs, &visible);
    let sbi = state.curr_sidebar_info(player);
    state.write_msg_buff("Welcome, adventurer!");   
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
            Cmd::ShowCharacterSheet => show_character_sheet(gui, player),
            Cmd::ShowInventory => show_inventory(gui, state, player, game_objs),
            Cmd::ToggleEquipment => toggle_equipment(state, player, gui),
            Cmd::Use => use_item(state, player, gui),
            Cmd::Quit => break,
            Cmd::Up => take_stairs(state, player, false),
            Cmd::WizardCommand => wiz_command(state, gui, player),
            _ => continue,
        }
        
        state.player_loc = player.location;

        if state.turn > start_turn {
            game_objs.do_npc_turns(state);
        }
        
        player.calc_vision_radius(state);
        
        let fov_start = Instant::now();
        let visible = fov::calc_fov(&state.map, player, FOV_HEIGHT, FOV_WIDTH);
        gui.v_matrix = fov_to_tiles(state, player, game_objs, &visible);        
        let fov_duration = fov_start.elapsed();
        println!("Time for fov: {:?}", fov_duration);
		
        // If anything wants an alert when it comes to end of turn...
        // for l in state.listeners.iter().filter(|i| i.1 == EventType::EndOfTurn) {
        //     println!("{:?}", l);
        // }

        let write_screen_start = Instant::now();
        let sbi = state.curr_sidebar_info(player);
        gui.write_screen(&mut state.msg_buff, Some(&sbi));
        let write_screen_duration = write_screen_start.elapsed();
        println!("Time for write_screen(): {:?}", write_screen_duration);        
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

    let mut game_objs = GameObjects::new();

    let dialogue_library = dialogue::read_dialogue_lib();
    
    let w = world::generate_world(&mut game_objs);
    
    let mut state = GameState::init(w.0, w.1);    
	
    title_screen(&mut gui);
    let player_name = who_are_you(&mut gui);

    let mut player = fetch_player(&state, &mut game_objs, &mut gui, player_name).unwrap();
    state.player_loc = player.location;

    let sbi = state.curr_sidebar_info(&player);
    gui.write_screen(&mut state.msg_buff, Some(&sbi));
    
    run(&mut gui, &mut state, &mut player, &mut game_objs, &dialogue_library);
}
