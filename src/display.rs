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

extern crate sdl2;

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use crate::game_obj::{GameObject, GameObjectDB};
use crate::map;
use crate::map::{Tile, DoorState};
use crate::util;

use super::{Cmd, GameState, FOV_WIDTH, FOV_HEIGHT};

use sdl2::event::Event;
use sdl2::EventPump;
use sdl2::keyboard::Mod;
use sdl2::keyboard::Keycode;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use sdl2::surface::Surface;
use sdl2::ttf::Font;
use sdl2::pixels::Color;

pub type Colour = (u8, u8, u8);

pub const BLACK: Colour = (0, 0, 0);
pub const WHITE: Colour = (255, 255, 255);
pub const GREY: Colour = (136, 136, 136);
pub const LIGHT_GREY: Colour = (220, 220, 220);
pub const DARK_GREY: Colour = (72, 73, 75);
pub const GREEN: Colour = (144, 238, 144);
pub const DARK_GREEN: Colour = (0, 71, 49);
pub const LIGHT_BROWN: Colour = (150, 75, 0);
pub const BROWN: Colour = (101, 67, 33);
pub const DARK_BROWN: Colour = (35, 18, 11);
pub const LIGHT_BLUE: Colour = (55, 198, 255);
pub const BLUE: Colour = (0, 0, 200);
pub const DARK_BLUE: Colour = (12, 35, 64);
pub const BEIGE: Colour = (255, 178, 127);
pub const BRIGHT_RED: Colour = (208, 28, 31);
pub const DULL_RED: Colour = (129, 12, 12);
pub const GOLD: Colour = (255, 215, 0);
pub const YELLOW: Colour = (255, 225, 53);
pub const YELLOW_ORANGE: Colour = (255, 159, 0);
pub const PINK: Colour = (255, 20, 147);
pub const HIGHLIGHT_PINK: Colour = (231, 84, 128);
pub const PURPLE: Colour = (138,43,226);
pub const LIGHT_PURPLE: Colour = (178, 102, 255);

const SCREEN_WIDTH: u32 = 58;
const SCREEN_HEIGHT: u32 = 25;
const BACKSPACE_CH: char = '\u{0008}';
const DEFAULT_FONT: &'static str = "DejaVuSansMono.ttf";
const SM_FONT_PT: u16 = 18;
const LG_FONT_PT: u16 = 25;

#[derive(Debug)]
pub struct SidebarInfo {
	name: String,
	ac: u8,
	curr_hp: u8,
	max_hp: u8,
	turn: u32,
	zorkmids: u32,
	weapon: String,
	curr_level: u8,
	poisoned: bool,
	confused: bool,
	paralyzed: bool,
}

impl SidebarInfo {
	pub fn new(name: String, curr_hp: u8, max_hp: u8, turn: u32, ac: u8, zorkmids: u32, weapon: String, curr_level: u8, poisoned: bool, confused: bool,
			paralyzed: bool) -> SidebarInfo {
		SidebarInfo { name, curr_hp, max_hp, turn, ac, zorkmids, weapon, curr_level, poisoned, confused, paralyzed, }
	}
}

fn tuple_to_sdl2_color(ct: &(u8, u8, u8)) -> Color {
	Color::RGBA(ct.0, ct.1, ct.2, 255)
}
 
pub struct GameUI<'a, 'b> {
	screen_width_px: u32,
	screen_height_px: u32,
	font_width: u32,
	font_height: u32,
	font: &'a Font<'a, 'b>,
	sm_font_width: u32,
	sm_font_height: u32,
	sm_font: &'a Font<'a, 'b>,
	canvas: WindowCanvas,
	event_pump: EventPump,
	pub v_matrix: [(Tile, bool); FOV_HEIGHT * FOV_WIDTH],
	surface_cache: HashMap<(char, Colour
	), Surface<'a>>,
	msg_line: String,
	messages: VecDeque<(String, bool)>,
	message_history: VecDeque<(String, u8)>,
}

impl<'a, 'b> GameUI<'a, 'b> {
	pub fn init(font: &'b Font, sm_font: &'b Font) -> Result<GameUI<'a, 'b>, String> {
		let (font_width, font_height) = font.size_of_char(' ').unwrap();
		let screen_width_px = SCREEN_WIDTH * font_width + 50;
		let screen_height_px = SCREEN_HEIGHT * font_height;

		let (sm_font_width, sm_font_height) = sm_font.size_of_char(' ').unwrap();

		let sdl_context = sdl2::init()?;
		let video_subsystem = sdl_context.video()?;
		let window = video_subsystem.window("rv 0.0.1", screen_width_px, screen_height_px)
			.position_centered()
			.opengl()
			.build()
			.map_err(|e| e.to_string())?;

		let v_matrix = [(map::Tile::Blank, false); FOV_WIDTH * FOV_HEIGHT];
		let canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
		let gui = GameUI { 
			screen_width_px, screen_height_px, 
			font, font_width, font_height, 
			canvas,
			event_pump: sdl_context.event_pump().unwrap(),
			sm_font, sm_font_width, sm_font_height,
			v_matrix,
			surface_cache: HashMap::new(),
			msg_line: "".to_string(),
			messages: VecDeque::new(),
			message_history: VecDeque::new(),
		};

		Ok(gui)
	}

	// I need to handle quitting the app actions here too
	fn wait_for_key_input(&mut self) -> Option<char> {
		loop {
			for event in self.event_pump.poll_iter() {
				match event {
					Event::TextInput { text:val, .. } => { 
						let ch = val.as_bytes()[0];
						return Some(ch as char);
					},
					Event::KeyDown {keycode: Some(Keycode::Return), .. } => return Some('\n'),
					Event::KeyDown {keycode: Some(Keycode::Backspace), .. } => return Some(BACKSPACE_CH),
					Event::KeyDown {keycode: Some(Keycode::Escape), .. } => return None,
					_ => { continue; }
				}
			}

			::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
		}
	}

	pub fn query_single_response(&mut self, question: &str, sbi: Option<&SidebarInfo>) -> Option<char> {
		self.draw_frame(question, sbi, true);
		self.wait_for_key_input()
	}

	pub fn query_yes_no(&mut self, question: &str, sbi: Option<&SidebarInfo>) -> char {
		loop {
			match self.query_single_response(question, sbi) {
				Some('y') => { return 'y'; },
				Some('n') | None => { return 'n'; },
				Some(_) => { continue; },
			}
		}
	}

	pub fn pick_direction(&mut self, msg: &str, sbi: Option<&SidebarInfo>) -> Option<(i32, i32)> {
		self.draw_frame(msg, sbi, true);

		loop {
			match self.wait_for_key_input() {
				Some('h') => { return Some((0, -1)); },
				Some('j') => { return Some((1, 0)); },
				Some('k') => { return Some((-1, 0)); },
				Some('l') => { return Some((0, 1)); },
				Some('y') => { return Some((-1, -1)); },
				Some('u') => { return Some((-1, 1)); },
				Some('b') => { return Some((1, -1)); },
				Some('n') => { return Some((1, 1)); },
				Some(_) => { continue; },
				None => { return None; },
			}
		}
	}

	pub fn query_natural_num(&mut self, query: &str, sbi: Option<&SidebarInfo>) -> Option<u32> {
		let mut answer = String::from("");

		loop {
			let mut s = String::from(query);
			s.push(' ');
			s.push_str(&answer);
			self.draw_frame(&s, sbi, true);

			match self.wait_for_key_input() {
				Some('\n') => { break; },
				Some(BACKSPACE_CH) => { answer.pop(); },
				Some(ch) => { 
					if ch >= '0' && ch <= '9' {
						answer.push(ch);
					}
				},
				None => { return None; },
			}
		}

		if answer.is_empty() {
			Some(0)
		} else {
			Some(answer.parse::<u32>().unwrap())
		}
	}

	pub fn query_user(&mut self, question: &str, max: u8, sbi: Option<&SidebarInfo>) -> Option<String> { 
		let mut answer = String::from("");

		loop {
			let mut s = String::from(question);
			s.push(' ');
			s.push_str(&answer);

			self.draw_frame(&s, sbi, true);
			
			match self.wait_for_key_input() {
				Some('\n') => { break; },
				Some(BACKSPACE_CH) => { answer.pop(); },
				Some(ch) => { 
					if answer.len() < max as usize { 
						answer.push(ch); 
					}
				},
				None => { return None; },
			}
		}

		Some(answer)
	}

	fn select_door(&mut self, prompt: &str, state: &GameState, game_obj_db: &mut GameObjectDB, door_state: DoorState) -> Option<(i32, i32, i8)> {	
		let player_loc = game_obj_db.get(0).unwrap().get_loc();
		if let Some(d) = map::adjacent_door(&state.map, player_loc, door_state) {
			Some(d)
		} else {
			self.select_dir(prompt, state, game_obj_db)
		}		
	}

	pub fn select_dir(&mut self, prompt: &str, state: &GameState, game_obj_db: &mut GameObjectDB) -> Option<(i32, i32, i8)> {
		match self.pick_direction(prompt, Some(&state.curr_sidebar_info(game_obj_db))) {
			Some(dir) => {
				let loc = game_obj_db.get(0).unwrap().get_loc();
				let obj_row =  loc.0 as i32 + dir.0;
				let obj_col = loc.1 as i32 + dir.1;
				let loc = (obj_row, obj_col, loc.2);
				Some(loc)
			},
			None => { 
				let sbi = state.curr_sidebar_info(game_obj_db);
				self.draw_frame("Nevermind.", Some(&sbi), true);
				None
			},
		}
	}

	pub fn select_target(&mut self, state: &GameState, game_obj_db: &mut GameObjectDB, prompt: &str) {
		let player_loc = game_obj_db.player().unwrap().get_loc();

		let mut npc_indexes = Vec::new();
		for i in 0..self.v_matrix.len() {
			if self.v_matrix[i].1 && self.v_matrix[i].0 != map::Tile::Blank {
				let loc = fov_coord_to_map_loc(i as i32, player_loc);
				if let Some(_) = game_obj_db.npc_at(&loc) {
					npc_indexes.push(i);
				}
			}
		}
		let mut npc_target = 0;

		let sbi = state.curr_sidebar_info(game_obj_db);
		let start = ((FOV_HEIGHT / 2) as i32, (FOV_WIDTH / 2) as i32);
		let mut loc = ((FOV_HEIGHT / 2) as i32, (FOV_WIDTH / 2) as i32);
		let orig_vmatrix = self.v_matrix.clone();
		let mut prev_line = vec![start];
		loop {
			let mut events = Vec::new();
			for event in self.event_pump.poll_iter() {
				events.push(event);
			}

			for event in events {
				match event {
					Event::KeyDown {keycode: Some(Keycode::Return), .. } => { return; },
					Event::KeyDown {keycode: Some(Keycode::Escape), .. } => { self.v_matrix = orig_vmatrix; return },
					Event::KeyDown {keycode: Some(Keycode::Tab), .. } => {
						if !npc_indexes.is_empty() {
							npc_target = (npc_target + 1 ) % npc_indexes.len();
							let i = npc_indexes[npc_target];
							let row = i / FOV_WIDTH;
							loc = (row as i32, (i - row * FOV_WIDTH) as i32);
						}
					},
					Event::TextInput { text:val, .. } => {
						let delta = if val == "k" {
							(-1, 0)
						} else if val == "j" {
							(1, 0)
						} else if val == "l" {
							(0, 1)
						} else if val == "h" {
							(0, -1)
						} else if val == "y" {
							(-1, -1)
						} else if val == "u" {
							(-1, 1)
						} else if val == "b" {
							(1, -1)
						} else {
							(1, 1)
						};

						let next = (loc.0 + delta.0, loc.1 + delta.1);
						let i = next.0 * FOV_WIDTH as i32 + next.1;						
						if i >= 0 && (i as usize) < orig_vmatrix.len() && orig_vmatrix[i as usize].0 != map::Tile::Blank {
							loc = next;
						}
					},
					_ => { println!("continue."); },
				}
			}

			let target_line = util::bresenham(start.0, start.1, loc.0, loc.1);
			if target_line != prev_line {
				let mut new_vm = orig_vmatrix.clone();
				for sq in target_line.iter().skip(1) {
					let i = sq.0 * FOV_WIDTH as i32 + sq.1;
					let vmi = &orig_vmatrix[i as usize];
					let sq_info = sq_info_for_tile(&vmi.0, vmi.1);
					new_vm[i as usize] = (map::Tile::Highlight(BLACK, HIGHLIGHT_PINK, sq_info.0), true);
				}
				self.v_matrix = new_vm;
				self.draw_frame(prompt, Some(&sbi), true);
				prev_line = target_line;
			}

			::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
		}
	}

	pub fn get_command(&mut self, state: &GameState, game_obj_db: &mut GameObjectDB) -> Cmd {
		loop {
			// I collect the events into a vector and then loop over them so that I can
			// call gui functions inside the event loop without Rust's fucking borrow checker
			// screeching at me.
			let mut events = Vec::new();
			for event in self.event_pump.poll_iter() {
				events.push(event);
			}

			for event in events {
				match event {
					Event::Quit {..} => { return Cmd::Quit },
					Event::KeyDown {keycode: Some(Keycode::H), keymod: Mod::LCTRLMOD, .. } |
					Event::KeyDown {keycode: Some(Keycode::H), keymod: Mod::RCTRLMOD, .. } => { 
						return Cmd::MsgHistory; 
					},
					Event::TextInput { text:val, .. } => {
						if val == "Q" {
							return Cmd::Quit;	
						} else if val == "i" {
							return Cmd::ShowInventory
						} else if val == "@" {
							return Cmd::ShowCharacterSheet;	
						} else if val == "e" {
							return Cmd::ToggleEquipment;
						} else if val == "." {
							return Cmd::Pass;
						} else if val == "S" {
							return Cmd::Save;
						} else if val == "B" {
							match self.select_dir("Bash what?", state, game_obj_db) {
								Some(loc) => return Cmd::Bash(loc),
								None => { },
							}
						} else if val == "C" {
							match self.select_dir("Chat with whom?", state, game_obj_db) {
								Some(loc) => return Cmd::Chat(loc),
								None => { },
							}
						} else if val == "a" {
                            return Cmd::Use;
                        } else if val == "?" {
							return Cmd::Help;
						} else if val == "o" {
							match self.select_door("Open what?", state, game_obj_db, DoorState::Closed) {
								Some(loc) => return Cmd::Open(loc),
								None => { },
							}							
						} else if val == "c" {
							match self.select_door("Close what?", state, game_obj_db, DoorState::Open) {
								Some(loc) => return Cmd::Close(loc),
								None => { },
							}	
						} else  if val == "k" {
							return Cmd::Move(String::from("N"));
						} else if val == "j" {
							return Cmd::Move(String::from("S"));
						} else if val == "l" {
							return Cmd::Move(String::from("E"));
						} else if val == "h" {
							return Cmd::Move(String::from("W"));
						} else if val == "y" {
							return Cmd::Move(String::from("NW"));
						} else if val == "u" {
							return Cmd::Move(String::from("NE"));
						} else if val == "b" {
							return Cmd::Move(String::from("SW"));
						} else if val == "n" {
							return Cmd::Move(String::from("SE"));
						} else if val == "," {
							return Cmd::PickUp;
						} else if val == "d" {
							return Cmd::DropItem;
						} else if val == "s" {
							return Cmd::Search;
						} else if val == "T" {
							self.select_target(state, game_obj_db, "Select target:");
							continue;
						} else if val == ">" {
							return Cmd::Down;
						} else if val == "<" {
							return Cmd::Up;
						} else if val == ":" {
							return Cmd::WizardCommand;
						} else if val == "@" {
							return Cmd::ShowCharacterSheet;
						}
					},
					_ => { continue },
				}
			}

			::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    	}
	}

	pub fn pause_for_more(&mut self) {
		loop {
			for event in self.event_pump.poll_iter() {
				// I need to handle a Quit/Exit event here	
				match event {
					Event::KeyDown {keycode: Some(Keycode::Escape), ..} |
					Event::KeyDown {keycode: Some(Keycode::Space), ..} => {
						// It seemed like the ' ' event was still in the queue.
						// I guess a TextInput event along with the KeyDown event?
						self.event_pump.poll_event();
						return;
					},
					_ => continue,
				}
			}

			::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
		}
	}

	fn write_line(&mut self, row: i32, line: &str, small_text: bool, text_colour: (u8, u8, u8)) {
		let fw: u32;
		let fh: u32;	
		let f: &Font;

		if small_text {
			f = self.sm_font;
			fw = self.sm_font_width;
			fh = self.sm_font_height;
		} else {
			f = self.font;
			fw = self.font_width;
			fh = self.font_height;
		}

		if line.len() == 0 {
			self.canvas
				.fill_rect(Rect::new(0, row * fh as i32, self.screen_width_px, fh))
				.expect("Error line!");

			return;
		}

		let surface = f.render(line)
			.blended(text_colour)
			.expect("Error rendering message line!");
		let texture_creator = self.canvas.texture_creator();
		let texture = texture_creator.create_texture_from_surface(&surface)
			.expect("Error create texture for messsage line!");
		let rect = Rect::new(2, row * fh as i32, line.len() as u32 * fw as u32, fh as u32);
		self.canvas.copy(&texture, None, Some(rect))
			.expect("Error copying message line texture to canvas!");
	}

	pub fn show_in_side_pane(&mut self, blurb: &str, lines: &Vec<(String, bool)>) -> Option<char> {
		self.canvas.clear();
		self.draw_frame(&"", None, false);
		self.write_line(0, blurb, false, WHITE);

		let panel_width = self.screen_height_px / 2;
		
		// clear the side panel
		self.canvas.set_draw_color(BLACK);
		self.canvas.fill_rect(Rect::new(panel_width as i32, self.font_height as i32, panel_width, self.screen_height_px)).unwrap();

		for j in 0..lines.len() {
			let text_colour = if lines[j].1 {
				WHITE
			} else {
				DARK_GREY
			};
			let rect = Rect::new(panel_width as  i32, j as i32 * self.sm_font_height as i32 + self.font_height as i32, lines[j].0.len() as u32 *  self.sm_font_width, self.sm_font_height);
			let surface = self.sm_font.render(&lines[j].0)
									  .shaded(text_colour, BLACK)
									  .expect("Error rendering line!");
													 
			let texture_creator = self.canvas.texture_creator();
			let texture = texture_creator.create_texture_from_surface(&surface)
										 .expect("Error creating texture!");
			self.canvas.copy(&texture, None, Some(rect))
				.expect("Error copying to canvas!");
		}

		self.canvas.present();
		self.wait_for_key_input()
	}

	// What I should do here but am not is make sure each line will fit on the
	// screen without being cut off. For the moment, I just gotta make sure any
	// lines don't have too many characterse. Something for a post 7DRL world
	// I guess.
	pub fn write_long_msg(&mut self, lines: &Vec<&str>, small_text: bool) {
		self.canvas.clear();
		
		// lines may contain strings at that are too wide for our screen, so we'll run through and check that 
		// before writing to the screen. Some of this code is more or less duplicated with similar functionality
		// in popup_msg() but I dunno if it's worth splitting it out.
		let width = SCREEN_WIDTH as usize + 10;
		let mut line_buff = Vec::new();
		for line in lines.iter() {
			if line.len() < width {
				line_buff.push(line.to_string());
			} else {
				let mut s = String::from("");
				for word in line.split(' ').into_iter() {
					if s.len() + word.len() > width {
						line_buff.push(String::from(s));
						s = String::from("");
					}
					s.push_str(word);
					s.push(' ');
				}
				if s.len() > 0 {
					line_buff.push(String::from(s));
				}
			}
		}

		let display_lines = FOV_HEIGHT as usize;
		let line_count = line_buff.len();
		let mut curr_line = 0;
		let mut curr_row = 0;
		while curr_line < line_count {
			self.write_line(curr_row as i32, &line_buff[curr_line], small_text, WHITE);
			curr_line += 1;
			curr_row += 1;

			if curr_row == display_lines - 2 && curr_line < line_count {
				self.write_line(curr_row as i32, "", small_text, WHITE);
				self.write_line(curr_row as i32 + 1, "-- Press space to continue --", small_text, WHITE);
				self.canvas.present();
				self.pause_for_more();
				curr_row = 0;
				self.canvas.clear();
			}
		}

		self.write_line(curr_row as i32, "", small_text, WHITE);
		self.write_line(curr_row as i32 + 1, "-- Press space to continue --", small_text, WHITE);
		self.canvas.present();
		self.pause_for_more();		
	}

	fn center_line_for_popup(&mut self, text: &str, width: u16) -> String {
		let mut line: String = "|".to_owned();
		let padding = (width as usize - 2) / 2 - (text.len() / 2);

		for _ in 0..padding {
			line.push(' ');
		}
		line.push_str(text);
		while line.len() < (width - 1) as usize {
			line.push(' ');
		}
		line.push('|');

		line
	}

	fn pad_line_for_popup(&mut self, text: &str, width: u16) -> String {
		let mut line: String = "| ".to_owned();
		line.push_str(text);

		while line.len() < (width - 1) as usize {
			line.push(' ');
		}
		line.push('|');

		line
	}

	pub fn popup_menu(&mut self, title: &str, text: &str, options: &HashSet<char>, sbi: Option<&SidebarInfo>) -> Option<char> {
		loop {
			if let Some(ch) = self.popup_msg(title, text, sbi) {
				if options.contains(&ch) {
					return Some(ch);
				}
			} else {
				return None;
			}			
		}
	}

	// I'll probably need to eventually add pagination but rendering the text into
	// lines was plenty for my brain for now...
	pub fn popup_msg(&mut self, title: &str, text: &str, sbi: Option<&SidebarInfo>) -> Option<char> {
		self.canvas.clear();
		self.draw_frame(&"", sbi, false);
		self.write_line(0, "", false, WHITE);

		let line_width = 45; // eventually this probably shouldn't be hardcoded here
		let r_offset = self.font_height as i32 * 3;
		let c_offset = self.font_width as i32 * 3;

		let mut lines = Vec::new();
		lines.push("+-------------------------------------------+".to_string());
		lines.push(self.center_line_for_popup(title, line_width));
		lines.push("|                                           |".to_string());

		// Easiest thing to do is to split the text into words and then append them to 
		// a line so long as there is room left on the current line.
		let words = util::split_msg(text);
		let mut wc = 0;
		let mut line = "".to_string();
		loop {
			if &words[wc] == "\n" {
				lines.push(self.pad_line_for_popup(&line, line_width));
				line = "".to_string();
				wc += 1;
			} else if line.len() + words[wc].len() < line_width as usize - 5 {
				line.push_str(&words[wc]);
				line.push(' ');
				wc += 1;
			} else {
				lines.push(self.pad_line_for_popup(&line, line_width));
				line = "".to_string();
			}

			if wc == words.len() {
				lines.push(self.pad_line_for_popup(&line, line_width));
				break;
			}
		}

		lines.push("|                                           |".to_string());
		lines.push("|                                           |".to_string());
		lines.push("+-------------------------------------------+".to_string());

		for j in 0..lines.len() {
			let rect = Rect::new(c_offset, r_offset + (self.sm_font_height as usize * j) as i32, self.sm_font_width * 45, self.sm_font_height);
			let surface = self.sm_font.render(&lines[j])
													 .shaded(WHITE, BLACK)
													 .expect("Error rendering line!");
													 
			let texture_creator = self.canvas.texture_creator();
			let texture = texture_creator.create_texture_from_surface(&surface)
										 .expect("Error creating texture!");
			self.canvas.copy(&texture, None, Some(rect))
				.expect("Error copying to canvas!");
		}

		self.canvas.present();
		self.wait_for_key_input()
	}

	fn write_sidebar_line(&mut self, line: &str, start_x: i32, row: usize, colour: sdl2::pixels::Color, indent: u8) {
		let surface = self.font.render(line)
			.blended(colour)
			.expect("Error rendering sidebar!");
		let texture_creator = self.canvas.texture_creator();
		let texture = texture_creator.create_texture_from_surface(&surface)
			.expect("Error creating texture for sdebar!");
		let rect = Rect::new(start_x + indent as i32 * self.font_width as i32, (self.font_height * row as u32) as i32, 
			line.len() as u32 * self.font_width, self.font_height);
		self.canvas.copy(&texture, None, Some(rect))
			.expect("Error copying sbi to canvas!");
	}

	fn write_sidebar(&mut self, sbi: &SidebarInfo) {
		let white = tuple_to_sdl2_color(&WHITE);
		let gold = tuple_to_sdl2_color(&GOLD);

		let fov_w = (FOV_WIDTH + 1) as i32 * self.font_width as i32; 
		self.write_sidebar_line(&sbi.name, fov_w, 1, white, 0);
		let s = format!("AC: {}", sbi.ac);
		self.write_sidebar_line(&s, fov_w, 2, white, 0);
		let s = format!("HP: {} ({})", sbi.curr_hp, sbi.max_hp);
		self.write_sidebar_line(&s, fov_w, 3, white, 0);

		self.write_sidebar_line("$", fov_w, 4, gold, 0);
		let s = format!(": {}", sbi.zorkmids);
		self.write_sidebar_line(&s, fov_w, 4, white, 1);

		if sbi.weapon.len() < 20 {
			self.write_sidebar_line(&sbi.weapon, fov_w, 5, white, 0);
		} else {
			// Dear Future Dana: please clean up this gross mess
			let words: Vec<&str> = sbi.weapon.split(' ').collect();
			let mut s = String::from(words[0]);
			let mut j = 1;
			while j < words.len() {
				if s.len() + words[j].len() + 1 < 20 {
					s.push(' ');
					s.push_str(words[j]);
				} else {
					break;
				}
				j += 1;
			}
			self.write_sidebar_line(&s, fov_w, 5, white, 0);

			if j < words.len() - 1 {
				let mut s = String::from(words[j]);
				j += 1;
				while j < words.len() {
					if s.len() + words[j].len() + 1 < 20 {
						s.push(' ');
						s.push_str(words[j]);
					} else {
						break;
					}
					j += 1;
				}
				self.write_sidebar_line(&s, fov_w, 6, white, 0);
			}
		}

		let mut effects_line = 19;
		if sbi.poisoned {
			self.write_sidebar_line("POISONED", fov_w, effects_line, tuple_to_sdl2_color(&GREEN), 0);
			effects_line -= 1;
		}
		if sbi.confused {
			self.write_sidebar_line("CONFUSED", fov_w, effects_line, tuple_to_sdl2_color(&PINK), 0);
			effects_line -= 1;
		}
		if sbi.paralyzed {
			self.write_sidebar_line("PARALYZED", fov_w, effects_line, tuple_to_sdl2_color(&BLUE), 0);
		}

		let s = if sbi.curr_level == 0 {
			"On the surface".to_string()
		} else {
			format!("Level {}", sbi.curr_level)
		};
		self.write_sidebar_line(&s, fov_w, 20, white, 0);		

		let s = format!("Turn: {}", sbi.turn);
		self.write_sidebar_line(&s, fov_w, 21, white, 0);		
	}

	fn draw_frame(&mut self, msg: &str, sbi: Option<&SidebarInfo>, render: bool) {
		if render {
			self.canvas.set_draw_color(BLACK);
			self.canvas.clear();
		}
		self.write_line(0, msg, false, WHITE);

		// I wonder if, since I've got rid of write_sq() and am generating a bunch of textures here,
		// if I can keep a texture_creator instance in the GUI struct and thereby placate Rust and 
		// keep a hashmap of textures. I should only have to generate a few since most times in view will
		// be repeated but still...
		let texture_creator = self.canvas.texture_creator();
		let mut textures = HashMap::new();
		let separator = sq_info_for_tile(&Tile::Separator, true);
		let separator_surface = self.font.render_char(separator.0)
										 .blended(separator.1)
										 .expect("Error creating character!");  
		let separator_texture = texture_creator.create_texture_from_surface(&separator_surface)
											   .expect("Error creating texture!");

		for row in 0..FOV_HEIGHT {
			for col in 0..FOV_WIDTH {
				let ti = sq_info_for_tile(&self.v_matrix[row * FOV_WIDTH + col].0, self.v_matrix[row * FOV_WIDTH + col].1);
				let (ch, fg_colour, bg_colour) = ti;

				//if !self.surface_cache.contains_key(&ti) {					
					// let surface = if ch == '#' {
					// 	self.font.render_char(ch)
					// 	//.blended(LIGHT_GREY)
					// 	.shaded(BLACK, char_colour)
					// 	.expect("Error creating character!")
					// } else {
					// 	self.font.render_char(ch)
					// 	.blended(char_colour)
					// 	.expect("Error creating character!")
					// };
				let surface = self.font.render_char(ch)
										.shaded(fg_colour, bg_colour)
										.expect("Error creating character");
				//	self.surface_cache.insert(ti, s);
				//}
				//let surface = self.surface_cache.get(&ti).unwrap();
				
				if !textures.contains_key(&ti) {
					let texture = texture_creator.create_texture_from_surface(&surface)
												 .expect("Error creating texture!");
					textures.insert(ti, texture);
				}

				let rect = Rect::new(col as i32 * self.font_width as i32, 
					(row as i32 + 1) * self.font_height as i32, self.font_width, self.font_height);
				self.canvas.copy(&textures[&ti], None, Some(rect))
					.expect("Error copying to canvas!");
			}
			let rect = Rect::new(FOV_WIDTH as i32 * self.font_width as i32, 
				(row as i32 + 1) * self.font_height as i32, self.font_width, self.font_height);
			self.canvas.copy(&separator_texture, None, Some(rect))
					.expect("Error copying to canvas!");			
		}

		if let Some(sidebar) = sbi {
			self.write_sidebar(sidebar);
		}

		// Draw the recent messages
		let msg_count = self.messages.len();
		if msg_count > 0  {
			let line = self.messages[0].0.to_string();
			let colour =  if self.messages[0].1 {
				WHITE
			} else {
				DARK_GREY
			};
			self.write_line((SCREEN_HEIGHT - 1) as i32, &line, false, colour);
		}
		if msg_count > 1 {
			let line = self.messages[1].0.to_string();
			let colour =  if self.messages[1].1 {
				WHITE
			} else {
				DARK_GREY
			};
			self.write_line((SCREEN_HEIGHT - 2) as i32, &line, false, colour);
		}
		if msg_count > 2 {
			let line = self.messages[2].0.to_string();
			let colour =  if self.messages[2].1 {
				WHITE
			} else {
				DARK_GREY
			};
			self.write_line((SCREEN_HEIGHT - 3) as i32, &line, false, colour);
		}

		if render {
			self.canvas.present();
		}
	}

	pub fn show_message_history(&mut self) {
		let mut history = Vec::new();
		for j in 0..self.message_history.len() {
			if self.message_history[j].1 == 1 {
				let s = self.message_history[j].0.to_string();
				history.push(s);
			} else {
				let s = format!("{} (x{})", self.message_history[j].0, self.message_history[j].1);
				history.push(s);
			}
			
		}
		
		let lines: Vec<&str> = history.iter().map(AsRef::as_ref).collect();
		self.write_long_msg(&lines, true);
	}

	pub fn update(&mut self, msg_queue: &mut VecDeque<String>, sbi: Option<&SidebarInfo>) {
		// Un-highlight the previous messages
		let mut j = 0;
		while j < self.messages.len() {
			self.messages[j].1 = false;
			j += 1;
			if j > 2 { break; }
		}
		
		let mut msg = "".to_string();		
		while !msg_queue.is_empty() {
			let item = msg_queue.pop_front().unwrap();
			
			if !self.message_history.is_empty() && self.message_history[0].0 == item {
				self.message_history[0].1 += 1;
			} else {
				self.message_history.push_front((item.clone(), 1));
				if self.message_history.len() > 60 {
					self.message_history.pop_back();
				}
			}

			if msg.len() + item.len() + 1 >=  SCREEN_WIDTH as usize - 2 {
				self.messages.push_front((msg, true));
				msg = "".to_string();
			}
			if !msg.is_empty() {
				msg.push(' ');
			}
			msg.push_str(&item);
		}

		if !msg.is_empty() {
			self.messages.push_front((msg, true));
		}
		self.draw_frame("", sbi, true);
	}

	pub fn clear_msg_buff(&mut self) {
		self.msg_line = "".to_string();
	}

	// Currently not handling a menu with more options than there are are lines on the screen...
	pub fn side_pane_menu(&mut self, preamble: String, menu: &Vec<(String, char)>, single_choice: bool) -> Option<HashSet<char>> {
		let mut answers: HashSet<char> = HashSet::new();
		let possible_answers: HashSet<char> = menu.iter().map(|m| m.1).collect();

		loop {
			let mut menu_items = Vec::new();
			for line in 0..menu.len() {
				let mut s = String::from("");				
				if answers.contains(&menu[line].1) {
					s.push_str(&String::from("\u{2713} "));					
				}
				s.push(menu[line].1);
				s.push_str(") ");
				s.push_str(&menu[line].0);
				menu_items.push((s.to_string(), true));
			}
	
			if !single_choice {
				menu_items.push((" ".to_string(), true));
				menu_items.push(("Select one or more options, then hit Return.".to_string(), true));
			}

			self.canvas.present();
			let answer = self.show_in_side_pane(&preamble, &menu_items);
			if single_choice {
				match answer {
					None => return None, 	// Esc was pressed, propagate it. 
											// Not sure if thers's a more Rustic way to do this
					Some(ch) => {
						if possible_answers.contains(&ch) {
							answers.insert(ch);
							return Some(answers);
						}	
					}
				}
			} else {
				match answer {
					None => return None, 	// Esc was pressed, propagate it. 
											// Not sure if thers's a more Rustic way to do this
					Some(ch) => {
						// * means select everything
						if ch == '*' {
							return Some(possible_answers);
						}

						if possible_answers.contains(&ch) {							
							if answers.contains(&ch) {
								answers.remove(&ch);
							} else {
								answers.insert(ch);
							}
						} else if ch == '\n' || ch == ' ' {
							break;
						}	
					}
				}
			}
		}
		
		Some(answers)
	}

	// A little different than menu_picker(), this is a screen with a lot of text that asks for 
	// the player to select options, but more free form (as opposed to just presenting a list of options)
	// This isn't yet handling someone hitting Esc, quitting the program, or just otherwise wanting to
	// bail out of the menu
	pub fn menu_wordy_picker(&mut self, menu: &Vec<&str>, answers: &HashSet<&char>) -> Option<char> {
		loop {
			self.canvas.clear();
			for line in 0..menu.len() {
				self.write_line(line as i32, &menu[line], true, WHITE);				
			}
	
			self.write_line(menu.len() as i32 + 1, "", true, WHITE);
			
			self.canvas.present();

			if let Some(choice) = self.wait_for_key_input() {
				if answers.contains(&choice) {
					return Some(choice);
				}
			}
		}
	}
}

fn fov_coord_to_map_loc(i: i32, player_loc: (i32, i32, i8)) -> (i32, i32, i8) {
	let centre_r = FOV_HEIGHT as i32 / 2;
	let centre_c = FOV_WIDTH as i32 / 2;
	let row = i / FOV_WIDTH as i32;	
	let fov_loc = (row  - centre_r, i - row * FOV_WIDTH as i32 - centre_c);

	(player_loc.0 + fov_loc.0, player_loc.1 + fov_loc.1 , player_loc.2)
}

fn sq_info_for_tile(tile: &map::Tile, lit: bool) -> (char, Colour, Colour) {
	match tile {
		map::Tile::Blank => (' ', BLACK, BLACK),
		map::Tile::Wall => {
			if lit {
				('#', BLACK, GREY)
			} else {
				('#', BLACK, DARK_GREY)
			}
		},
		map::Tile::LitWall(colour) => {
			if lit { 
				('#', BLACK, *colour)
			} else {
				('#', BLACK, DARK_GREY)
			}
		},
		map::Tile::WoodWall => {
			if lit {
				('#', BLACK, BROWN)
			} else {
				('#', BLACK, DARK_BROWN)
			}
		},
		map::Tile::Tree => {
			if lit {
				('\u{03D9}', GREEN, BLACK)
			}
			else {
				('\u{03D9}', DARK_GREEN, BLACK)
			}
		},
		map::Tile::Dirt => {
			if lit {
				('.', LIGHT_BROWN, BLACK)
			} else {
				('.', BROWN, BLACK)
			}
		},
		map::Tile::Bridge => {
			if lit {
				('=', DARK_GREY, BLACK)
			} else {
				('=', DARK_GREY, BLACK)
			}
		},
		map::Tile::Door(DoorState::Closed) | map::Tile::Door(DoorState::Locked) => {
			if lit {
				('+', LIGHT_BROWN, BLACK)
			} else {
				('+', BROWN, BLACK)
			}
		},				
		map::Tile::Door(DoorState::Open) | map::Tile::Door(DoorState::Broken) => {
			if lit {
				('/', LIGHT_BROWN, BLACK)
			} else {
				('/', BROWN, BLACK)
			}
		},
		map::Tile::Grass => {
			if lit {
				(',', GREEN, BLACK)
			}
			else {
				(',', DARK_GREEN, BLACK)
			}
		},
		map::Tile::Player(colour) => ('@', *colour, BLACK),
		map::Tile::Water => {
			if lit {
				('}', LIGHT_BLUE, BLACK)
			} else {
				('}', BLUE, BLACK)
			}
		},
		map::Tile::DeepWater | map::Tile::UndergroundRiver => {
			if lit {
				('}', BLUE, BLACK)
			} else {
				('}', DARK_BLUE, BLACK)
			}
		},
		map::Tile::WorldEdge => {
			if lit {
				('}', BLUE, BLACK)
			} else {
				('}', DARK_BLUE, BLACK)
			}
		},
		map::Tile::Sand => ('.', BEIGE, BLACK),
		map::Tile::StoneFloor => {
			if lit {
				('.', GREY, BLACK)
			} else {
				('.', DARK_GREY, BLACK)
			}
		},
		map::Tile::ColourFloor(colour) => ('.', *colour, BLACK),
		map::Tile::Mountain => {
			if lit {
				('\u{039B}', GREY, BLACK)
			} else {
				('\u{039B}', DARK_GREY, BLACK)
			}
		},
		map::Tile::SnowPeak => {
			if lit {
				('\u{039B}', WHITE, BLACK)
			} else {
				('\u{039B}', GREY, BLACK)
			}		
		},
		map::Tile::Lava => {
			if lit {
				('{', BRIGHT_RED, BLACK)
			} else {
				('{', DULL_RED, BLACK)
			}
		},
		map::Tile::Gate(DoorState::Closed) | map::Tile::Gate(DoorState::Locked) => { 
			if lit { 
				('#', LIGHT_BLUE, BLACK)
			} else {
				('#', LIGHT_GREY, BLACK)
			}
		},
		map::Tile::Gate(DoorState::Open) | map::Tile::Gate(DoorState::Broken) => { 
			if lit { 
				('/', LIGHT_BLUE, BLACK)
			} else {
				('/', LIGHT_GREY, BLACK)
			}
		},
		map::Tile::Creature(colour, ch) => (*ch, *colour, BLACK),
		map::Tile::Thing(lit_colour, unlit_colour, ch) => {
			if lit {
			 	(*ch, *lit_colour, BLACK)
			} else {
				(*ch, *unlit_colour, BLACK)
			}
		},
		map::Tile::Separator => ('|', WHITE, BLACK),
		map::Tile::Bullet(ch) => (*ch, WHITE, BLACK),
		map::Tile::OldFirePit(_) => {
			if lit {
				('#', LIGHT_GREY, BLACK)
			} else {
				('#', GREY, BLACK)
			}
		},
		map::Tile::FirePit => {
			if lit {
				('#', BRIGHT_RED, BLACK)
			} else {
				('#', DULL_RED, BLACK)
			}
		},
		map::Tile::Forge => {
			if lit {
				('^', BRIGHT_RED, BLACK)
			} else {
				('^', DULL_RED, BLACK)
			}
		},
		map::Tile::Floor => {
			if lit {
				('.', BEIGE, BLACK)
			} else {
				('.', BROWN, BLACK)
			}
		},
		map::Tile::Window(ch) => {
			if lit {
				(*ch, LIGHT_BROWN, BLACK)
			} else {
				(*ch, BROWN, BLACK)
			}
		},
		map::Tile::Spring => {
			if lit {
				('~', LIGHT_BLUE, BLACK)
			} else {
				('~', BLUE, BLACK)
			}
		},
		map::Tile::Portal => {
			if lit {
				('Ո', GREY, BLACK)
			} else {
				('Ո', DARK_GREY, BLACK)
			}
		},
		map::Tile::Fog => ('#', LIGHT_GREY, BLACK),
		map::Tile::StairsUp => {
			if lit {
				('<', GREY, BLACK)
			} else {
				('<', DARK_GREY, BLACK)
			}
		},
		map::Tile::StairsDown => {
			if lit {
				('>', GREY, BLACK)
			} else {
				('>', DARK_GREY, BLACK)
			}
		},
		map::Tile::Shrine(_) => {
			if lit {
				('_', LIGHT_GREY, BLACK)
			} else {
				('_', GREY, BLACK)
			}
		},
		map::Tile::Trigger => {
			if lit {
				('.', DARK_GREY, BLACK)
			} else {
				('.', DARK_GREY, BLACK)
			}
		},
		map::Tile::TeleportTrap => {
			if lit {
				('^', PINK, BLACK)
			} else {
				('^', PURPLE, BLACK)
			}
		},
		map::Tile::Well => {
			if lit {
				('~', BLUE, BLACK)
			} else {
				('~', DARK_BLUE, BLACK)
			}
		},
		map::Tile::Highlight(fg_colour, bg_colour, ch) => (*ch, *fg_colour, *bg_colour),
	}
}
