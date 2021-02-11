#![allow(dead_code)]

extern crate rand;
extern crate sdl2;

mod display;
mod map;

use crate::display::{GameUI};

use std::collections::{VecDeque};
//use std::io::prelude::*;
//use std::fs;
//use std::fs::File;
use std::path::Path;

use rand::Rng;

const MSG_HISTORY_LENGTH: usize = 50;
const FOV_WIDTH: usize = 41;
const FOV_HEIGHT: usize = 21;

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
	TurnWheelClockwise,
	TurnWheelAnticlockwise,	
	ToggleAnchor,
	ToggleHelm,
	Quaff,
	FireGun,
	Reload,
	WorldMap,
	Search,
	Read,
	Eat,
	Save,
    EnterPortal,
	Chat,
    Use,
	Help,
}

pub struct GameState {
	msg_buff: VecDeque<String>,
	msg_history: VecDeque<(String, u32)>,
    turn: u32,
    vision_radius: u8,
}

impl GameState {
    pub fn init() -> GameState {
        let state = GameState {
            msg_buff: VecDeque::new(),
            msg_history: VecDeque::new(),
            turn: 0,
            vision_radius: 3,
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

    pub fn calc_vision_radius(&mut self) {
        let prev_vr = self.vision_radius;
        let curr_time = (self.turn / 100 + 12) % 24;
        self.vision_radius = if curr_time >= 6 && curr_time <= 19 {
            99
        } else if curr_time >= 20 && curr_time <= 21 {
            9
        } else if curr_time >= 21 && curr_time <= 23 {
            7
        } else if curr_time < 4 {
            5
        } else if curr_time >= 4 && curr_time < 5 {
            7
        } else {
            9
        };

        if prev_vr == 99 && self.vision_radius == 9 {
            self.write_msg_buff("The sun is beginning to set.");
        }
        if prev_vr == 5 && self.vision_radius == 7 {
            self.write_msg_buff("Sunrise soon.");
        }
    }
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

fn run(gui: &mut GameUI, state: &mut GameState) {
    state.write_msg_buff("Hello, world?");

    loop {
        let size = FOV_HEIGHT * FOV_WIDTH;
        let mut v_matrix = vec![map::Tile::Blank; size];

        for j in 0..size {
            if rand::thread_rng().gen_range(0.0, 1.0) < 0.5 {
                v_matrix[j] = map::Tile::Tree;
            } else {
                v_matrix[j] = map::Tile::Grass;
            }
        }        
        gui.v_matrix = v_matrix;

        gui.write_screen(&mut state.msg_buff);

        let start_turn = state.turn;
        let cmd = gui.get_command(&state);
        match cmd {
            Cmd::Quit => break,
            Cmd::Pass => state.turn += 1,
            Cmd::Chat => {
                gui.popup_msg("Dale, the Innkeeper", "Welcome to Skara Brae, stranger! You'll find the dungeon in the foothills but watch out for goblins on the way!");
                state.turn += 1
            },
            Cmd::PickUp => {
                let s = gui.query_user("Yes, what?", 15).unwrap();
                println!("result: {}", s);
            },
            _ => continue,
        }
        
        gui.write_screen(&mut state.msg_buff);
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

    title_screen(&mut gui);
    gui.write_screen(&mut state.msg_buff);
    run(&mut gui, &mut state);
}
