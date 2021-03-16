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

use crate::game_obj::GameObjects;
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

pub const BLACK: (u8, u8, u8) = (0, 0, 0);
pub const WHITE: (u8, u8, u8) = (255, 255, 255);
pub const GREY: (u8, u8, u8) = (136, 136, 136);
pub const LIGHT_GREY: (u8, u8, u8) = (220, 220, 220);
pub const DARK_GREY: (u8, u8, u8) = (72, 73, 75);
pub const GREEN: (u8, u8, u8) = (144, 238, 144);
pub const DARK_GREEN: (u8, u8, u8) = (0, 71, 49);
pub const LIGHT_BROWN: (u8, u8, u8) = (150, 75, 0);
pub const BROWN: (u8, u8, u8) = (101, 67, 33);
pub const DARK_BROWN: (u8, u8, u8) = (35, 18, 11);
pub const LIGHT_BLUE: (u8, u8, u8) = (55, 198, 255);
pub const BLUE: (u8, u8, u8) = (0, 0, 200);
pub const DARK_BLUE: (u8, u8, u8) = (12, 35, 64);
pub const BEIGE: (u8, u8, u8) = (255, 178, 127);
pub const BRIGHT_RED: (u8, u8, u8) = (208, 28, 31);
pub const DULL_RED: (u8, u8, u8) = (129, 12, 12);
pub const GOLD: (u8, u8, u8) = (255, 215, 0);
pub const YELLOW: (u8, u8, u8) = (255, 225, 53);
pub const YELLOW_ORANGE: (u8, u8, u8,) = (255, 159, 0);
pub const PINK: (u8, u8, u8) = (255, 20, 147);
pub const PURPLE: (u8, u8, u8) = (138,43,226);

const SCREEN_WIDTH: u32 = 58;
const SCREEN_HEIGHT: u32 = 22;
const BACKSPACE_CH: char = '\u{0008}';
const DEFAULT_FONT: &'static str = "DejaVuSansMono.ttf";
const SM_FONT_PT: u16 = 18;
const LG_FONT_PT: u16 = 24;

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
}

impl SidebarInfo {
	pub fn new(name: String, curr_hp: u8, max_hp: u8, turn: u32, ac: u8, zorkmids: u32, weapon: String, curr_level: u8) -> SidebarInfo {
		SidebarInfo { name, curr_hp, max_hp, turn, ac, zorkmids, weapon, curr_level }
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
	pub v_matrix: Vec<(map::Tile, bool)>,
	surface_cache: HashMap<(char, Color), Surface<'a>>,
	msg_line: String,
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

		let v_matrix = vec![(map::Tile::Blank, false); FOV_WIDTH * FOV_HEIGHT];
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
		self.draw_frame(question, sbi);
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
		self.draw_frame(msg, sbi);

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
			self.draw_frame(&s, sbi);

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

			self.draw_frame(&s, sbi);
			
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

	fn select_door(&mut self, prompt: &str, state: &GameState, game_objs: &mut GameObjects, door_state: DoorState) -> Option<(i32, i32, i8)> {	
		let player_loc = game_objs.player_location();
		if let Some(d) = map::adjacent_door(&state.map, player_loc, door_state) {
			Some(d)
		} else {
			self.select_dir(prompt, state, game_objs)
		}		
	}

	fn select_dir(&mut self, prompt: &str, state: &GameState, game_objs: &mut GameObjects) -> Option<(i32, i32, i8)> {
		match self.pick_direction(prompt, Some(&state.curr_sidebar_info(game_objs))) {
			Some(dir) => {
				let loc = game_objs.player_location();
				let obj_row =  loc.0 as i32 + dir.0;
				let obj_col = loc.1 as i32 + dir.1;
				let loc = (obj_row, obj_col, loc.2);
				Some(loc)
			},
			None => { 
				let sbi = state.curr_sidebar_info(game_objs);
				self.draw_frame("Nevermind.", Some(&sbi));
				None
			},
		}
	}

	pub fn get_command(&mut self, state: &GameState, game_objs: &mut GameObjects) -> Cmd {
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
						} else if val == "w" {
							return Cmd::ToggleEquipment;
						} else if val == "." {
							return Cmd::Pass;
						} else if val == "q" {
							return Cmd::Quaff;
						} else if val == "f" {
							return Cmd::FireGun;
						} else if val == "r" {
							return Cmd::Reload;						
						} else if val == "R" {
							return Cmd::Read;
						} else if val == "E" {
							return Cmd::Eat;
						} else if val == "S" {
							return Cmd::Save; 
						} else if val == "C" {
							match self.select_dir("Chat with whom?", state, game_objs) {
								Some(loc) => return Cmd::Chat(loc),
								None => { },
							}
						} else if val == "U" {
                            return Cmd::Use;
                        } else if val == "?" {
							return Cmd::Help;
						} else if val == "o" {
							match self.select_door("Open what?", state, game_objs, DoorState::Closed) {
								Some(loc) => return Cmd::Open(loc),
								None => { },
							}							
						} else if val == "c" {
							match self.select_door("Close what?", state, game_objs, DoorState::Open) {
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

	fn write_line(&mut self, row: i32, line: &str, small_text: bool) {
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
			.blended(WHITE)
			.expect("Error rendering message line!");
		let texture_creator = self.canvas.texture_creator();
		let texture = texture_creator.create_texture_from_surface(&surface)
			.expect("Error create texture for messsage line!");
		let rect = Rect::new(2, row * fh as i32, line.len() as u32 * fw as u32, fh as u32);
		self.canvas.copy(&texture, None, Some(rect))
			.expect("Error copying message line texture to canvas!");
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
			self.write_line(curr_row as i32, &line_buff[curr_line], small_text);
			curr_line += 1;
			curr_row += 1;

			if curr_row == display_lines - 2 && curr_line < line_count {
				self.write_line(curr_row as i32, "", small_text);
				self.write_line(curr_row as i32 + 1, "-- Press space to continue --", small_text);
				self.canvas.present();
				self.pause_for_more();
				curr_row = 0;
				self.canvas.clear();
			}
		}

		self.write_line(curr_row as i32, "", small_text);
		self.write_line(curr_row as i32 + 1, "-- Press space to continue --", small_text);
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

	pub fn popup_menu(&mut self, title: &str, text: &str, options: HashSet<char>) -> Option<char> {
		loop {
			if let Some(ch) = self.popup_msg(title, text) {
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
	pub fn popup_msg(&mut self, title: &str, text: &str) -> Option<char> {
		self.write_line(0, "", false);

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
		self.write_sidebar_line(&sbi.weapon, fov_w, 5, white, 0);

		let s = if sbi.curr_level == 0 {
			"On the surface".to_string()
		} else {
			format!("Level {}", sbi.curr_level)
		};
		self.write_sidebar_line(&s, fov_w, 20, white, 0);		

		let s = format!("Turn: {}", sbi.turn);
		self.write_sidebar_line(&s, fov_w, 21, white, 0);		
	}

	fn draw_frame(&mut self, msg: &str, sbi: Option<&SidebarInfo>) {
		self.canvas.set_draw_color(BLACK);
		self.canvas.clear();

		self.write_line(0, msg, false);

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
				let (ch, char_colour) = ti;

				if !self.surface_cache.contains_key(&ti) {
					let s = self.font.render_char(ch)
									 .blended(char_colour)
									 .expect("Error creating character!");  
					self.surface_cache.insert(ti, s);
				}
				let surface = self.surface_cache.get(&ti).unwrap();
				
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

		self.canvas.present();
	}

	pub fn write_screen(&mut self, msgs: &VecDeque<String>, sbi: Option<&SidebarInfo>) {
		let mut msg = String::from("");
		if !msgs.is_empty() {
			let mut words = VecDeque::new();
			if !self.msg_line.is_empty() {
				for w in self.msg_line.split(' ') {
					let s = String::from(w);
					words.push_back(s);
				}
			}
			for line in msgs.iter() {
				for w in line.split(' ') {
					let s = String::from(w);
					words.push_back(s);
				}
			}

			while !words.is_empty() {
				let word = words.pop_front().unwrap();
				
				// If we can't fit the new word in the message put it back
				// on the queue and display what we have so far
				if msg.len() + word.len() + 1 >=  SCREEN_WIDTH as usize - 9 {
					words.push_front(word);
					msg.push_str("--More--");
					self.draw_frame(&msg, sbi);
					self.pause_for_more();
					msg = String::from("");
				} else {
					msg.push_str(&word);
					msg.push(' ');
				}
			}
		} else {
			msg = self.msg_line.clone();
		}

		self.draw_frame(&msg, sbi);
		if !msg.is_empty() {			
			self.msg_line = msg;
		}
	}

	pub fn clear_msg_buff(&mut self) {
		self.msg_line = "".to_string();
	}

	// Currently not handling a menu with more options than there are are lines on the screen...
	pub fn menu_picker(&mut self, preamble: String, menu: &Vec<(String, char)>, single_choice: bool, small_font: bool) -> Option<HashSet<char>> {
		let mut answers: HashSet<char> = HashSet::new();
		let possible_answers: HashSet<char> = menu.iter().map(|m| m.1).collect();

		loop {
			self.canvas.clear();
			self.write_line(0, &preamble, small_font);

			for line in 0..menu.len() {
				let mut s = String::from("");				
				if answers.contains(&menu[line].1) {
					s.push_str(&String::from("\u{2713} "));					
				}
				s.push(menu[line].1);
				s.push_str(") ");
				s.push_str(&menu[line].0);
				self.write_line(line as i32 + 1, &s, small_font);
			}
	
			self.write_line(menu.len() as i32 + 1, "", small_font);	
			if !single_choice {
				self.write_line(menu.len() as i32 + 2, "Select one or more options, then hit Return.", small_font);	
			}

			self.canvas.present();

			let answer = self.wait_for_key_input();			
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
				self.write_line(line as i32, &menu[line], true);				
			}
	
			self.write_line(menu.len() as i32 + 1, "", true);	
			
			self.canvas.present();

			if let Some(choice) = self.wait_for_key_input() {
				if answers.contains(&choice) {
					return Some(choice);
				}
			}
		}
	}
}

fn sq_info_for_tile(tile: &map::Tile, lit: bool) -> (char, sdl2::pixels::Color) {
	match tile {
		map::Tile::Blank => (' ', tuple_to_sdl2_color(&BLACK)),
		map::Tile::Wall => {
			if lit {
				('#', tuple_to_sdl2_color(&GREY))
			} else {
				('#', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::WoodWall => {
			if lit {
				('#', tuple_to_sdl2_color(&BROWN))
			} else {
				('#', tuple_to_sdl2_color(&DARK_BROWN))
			}
		},
		map::Tile::Tree => {
			if lit {
				('\u{03D9}', tuple_to_sdl2_color(&GREEN))
			}
			else {
				('\u{03D9}', tuple_to_sdl2_color(&DARK_GREEN))
			}
		},
		map::Tile::Dirt => {
			if lit {
				('.', tuple_to_sdl2_color(&LIGHT_BROWN))
			} else {
				('.', tuple_to_sdl2_color(&BROWN))
			}
		},
		map::Tile::Bridge => {
			if lit {
				('=', tuple_to_sdl2_color(&DARK_GREY))
			} else {
				('=', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::Door(DoorState::Closed) | map::Tile::Door(DoorState::Locked) => {
			if lit {
				('+', tuple_to_sdl2_color(&LIGHT_BROWN))
			} else {
				('+', tuple_to_sdl2_color(&BROWN))
			}
		},				
		map::Tile::Door(DoorState::Open) | map::Tile::Door(DoorState::Broken) => {
			if lit {
				('/', tuple_to_sdl2_color(&LIGHT_BROWN))
			} else {
				('/', tuple_to_sdl2_color(&BROWN))
			}
		},
		map::Tile::Grass => {
			if lit {
				(',', tuple_to_sdl2_color(&GREEN))
			}
			else {
				(',', tuple_to_sdl2_color(&DARK_GREEN))
			}
		},
		map::Tile::Player(colour) => ('@', tuple_to_sdl2_color(colour)),
		map::Tile::Water => {
			if lit {
				('}', tuple_to_sdl2_color(&LIGHT_BLUE))
			} else {
				('}', tuple_to_sdl2_color(&BLUE))
			}
		},
		map::Tile::DeepWater | map::Tile::UndergroundRiver => {
			if lit {
				('}', tuple_to_sdl2_color(&BLUE))
			} else {
				('}', tuple_to_sdl2_color(&DARK_BLUE))
			}
		},
		map::Tile::WorldEdge => {
			if lit {
				('}', tuple_to_sdl2_color(&BLUE))
			} else {
				('}', tuple_to_sdl2_color(&DARK_BLUE))
			}
		},
		map::Tile::Sand => ('.', tuple_to_sdl2_color(&BEIGE)),
		map::Tile::StoneFloor => {
			if lit {
				('.', tuple_to_sdl2_color(&GREY))
			} else {
				('.', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::ColourFloor(c) => ('.', tuple_to_sdl2_color(&c)),
		map::Tile::Mountain => {
			if lit {
				('\u{039B}', tuple_to_sdl2_color(&GREY))
			} else {
				('\u{039B}', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::SnowPeak => {
			if lit {
				('\u{039B}', tuple_to_sdl2_color(&WHITE))
			} else {
				('\u{039B}', tuple_to_sdl2_color(&GREY))
			}		
		},
		map::Tile::Lava => {
			if lit {
				('{', tuple_to_sdl2_color(&BRIGHT_RED))
			} else {
				('{', tuple_to_sdl2_color(&DULL_RED))
			}
		},
		map::Tile::Gate(DoorState::Closed) | map::Tile::Gate(DoorState::Locked) => { 
			if lit { 
				('#', tuple_to_sdl2_color(&LIGHT_BLUE)) 
			} else {
				('#', tuple_to_sdl2_color(&LIGHT_GREY))
			}
		},
		map::Tile::Gate(DoorState::Open) | map::Tile::Gate(DoorState::Broken) => { 
			if lit { 
				('/', tuple_to_sdl2_color(&LIGHT_BLUE)) 
			} else {
				('/', tuple_to_sdl2_color(&LIGHT_GREY))
			}
		},
		map::Tile::Creature(colour, ch) => (*ch, tuple_to_sdl2_color(colour)),
		map::Tile::Thing(lit_colour, unlit_colour, ch) => {
			if lit {
			 	(*ch, tuple_to_sdl2_color(lit_colour))
			} else {
				(*ch, tuple_to_sdl2_color(unlit_colour))
			}
		},
		map::Tile::Separator => ('|', tuple_to_sdl2_color(&WHITE)),
		map::Tile::Bullet(ch) => (*ch, tuple_to_sdl2_color(&WHITE)),
		map::Tile::OldFirePit(_) => {
			if lit {
				('#', tuple_to_sdl2_color(&LIGHT_GREY))
			} else {
				('#', tuple_to_sdl2_color(&GREY))
			}
		},
		map::Tile::FirePit => {
			if lit {
				('#', tuple_to_sdl2_color(&BRIGHT_RED))
			} else {
				('#', tuple_to_sdl2_color(&DULL_RED))
			}
		},
		map::Tile::Floor => {
			if lit {
				('.', tuple_to_sdl2_color(&BEIGE))
			} else {
				('.', tuple_to_sdl2_color(&BROWN))
			}
		},
		map::Tile::Window(ch) => {
			if lit {
				(*ch, tuple_to_sdl2_color(&LIGHT_BROWN))
			} else {
				(*ch, tuple_to_sdl2_color(&BROWN))
			}
		},
		map::Tile::Spring => {
			if lit {
				('~', tuple_to_sdl2_color(&LIGHT_BLUE))
			} else {
				('~', tuple_to_sdl2_color(&BLUE))
			}
		},
		map::Tile::Portal => {
			if lit {
				('Ո', tuple_to_sdl2_color(&GREY))
			} else {
				('Ո', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::Fog => ('#', tuple_to_sdl2_color(&LIGHT_GREY)),
		map::Tile::BoulderTrap(colour, hidden, _, _, _) => {
			if *hidden {
				('.', tuple_to_sdl2_color(colour))
			} else {
				('^', tuple_to_sdl2_color(&WHITE))
			}
		},
		map::Tile::StairsUp => {
			if lit {
				('<', tuple_to_sdl2_color(&GREY))
			} else {
				('<', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::StairsDown => {
			if lit {
				('>', tuple_to_sdl2_color(&GREY))
			} else {
				('>', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::Shrine(_) => {
			if lit {
				('_', tuple_to_sdl2_color(&LIGHT_GREY))
			} else {
				('_', tuple_to_sdl2_color(&GREY))
			}
		},
		map::Tile::Trigger => {
			if lit {
				('.', tuple_to_sdl2_color(&DARK_GREY))
			} else {
				('.', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::TeleportTrap => {
			if lit {
				('^', tuple_to_sdl2_color(&PINK))
			} else {
				('^', tuple_to_sdl2_color(&PURPLE))
			}
		}
		map::Tile::Rubble => {
			if lit {
				(':', tuple_to_sdl2_color(&GREY))
			} else {
				(':', tuple_to_sdl2_color(&DARK_GREY))
			}
		},
		map::Tile::Well => {
			if lit {
				('~', tuple_to_sdl2_color(&BLUE))
			} else {
				('~', tuple_to_sdl2_color(&DARK_BLUE))
			}
		},
	}
}
