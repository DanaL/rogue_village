// This file is part of RogueVillage, a roguelike game.
//
// YarrL is free software: you can redistribute it and/or modify
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

// Some miscellaneous structs and functions used in a few plces

use crate::game_obj::GameObjectDB;

pub const ADJ: [(i32, i32); 8] = [(0, -1), (0, 1), (-1, 0), (1, 0), (-1, -1), (-1, 1), (1, -1), (1, 1)];

pub fn num_to_nth(n: u8) -> String {
	let x = n % 10;

	match x {
		1 => format!("{}st", n),
		2 => format!("{}nd", n),
		3 => format!("{}rd", n),
		_ => format!("{}th", n),
	}
}

pub fn split_msg(text: &str) -> Vec<String> {
	let mut words = Vec::new();

	let mut word = "".to_string();		
	for c in text.chars() {
		if c == ' ' && !word.is_empty() {
			words.push(word.to_string());
			word = "".to_string();
		} else if c == '\n' {
			if !word.is_empty() {
				words.push(word.to_string());
			}
			words.push("\n".to_string());
			word = "".to_string();
		} else {
			word.push(c);
		}
	}

	if !word.is_empty() {
		words.push(word.to_string());
	}

	words
}

pub fn distance(x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
	let d = (i32::abs(i32::pow(x1 - x2, 2)) + i32::abs(i32::pow(y1 - y2, 2))) as f64;
	return d.sqrt();
}

pub fn are_adj(a: (i32, i32, i8), b: (i32, i32, i8)) -> bool {
	for adj in ADJ.iter() {
		// Note that so far in the game, I don't want squares that are at the same row/col but on different floors 
		// to count as adjacent
		if (a.0 + adj.0, a.1 + adj.1, a.2) == b {
			return true;
		}
	}

	false
}

// Bresenham functions straight out of my old scientific computing textbook
pub fn bresenham(r0: i32, c0: i32, r1: i32, c1: i32) -> Vec<(i32, i32)> {
	let mut pts = Vec::new();
	let mut error = 0;
	let mut r = r0;
	let mut c = c0;
	let mut delta_c = c1 - c0;
	
	let step_c = if delta_c < 0 {
		delta_c = -delta_c;
		-1
	} else {
		1
	};

	let mut delta_r = r1 - r0;
	let step_r = if delta_r < 0 {
		delta_r = -delta_r;
		-1
	} else {
		1
	};		

	if delta_r <= delta_c {
		let criterion = delta_c / 2;
		while c != c1 + step_c {
			pts.push((r, c));
			c += step_c;
			error += delta_r;
			if error > criterion {
				error -= delta_c;
				r += step_r;
			}
		}
	} else {
		let criterion = delta_r / 2;
		while r != r1 + step_r {
			pts.push((r, c));
			r += step_r;
			error += delta_c;
			if error > criterion {
				error -= delta_r;
				c += step_c;
			}
		}
	}

	pts
}

pub fn bresenham_circle(rc: i32, cc: i32, radius: i32) -> Vec<(i32, i32)> {
	let mut pts = Vec::new();
	let mut x = radius;
	let mut y = 0;
	let mut error = 0;

	let mut sqrx_inc = 2 * radius - 1; 
	let mut sqry_inc = 1;

	while y <= x {
		pts.push((rc + y, cc + x));
		pts.push((rc + y, cc - x));
		pts.push((rc - y, cc + x));
		pts.push((rc - y, cc - x));
		pts.push((rc + x, cc + y));
		pts.push((rc + x, cc - y));
		pts.push((rc - x, cc + y));
		pts.push((rc - x, cc - y));
	
		y += 1;
		error += sqry_inc;
		sqry_inc += 2;
		if error > x {
			x -= 1;
			error -= sqrx_inc;
			sqrx_inc -= 2;
		}	
	}

	pts
}

fn conjugate(second_person: bool, verb: &str) -> String {
	if second_person {
		if verb == "to be" { return "are".to_string() }
		
		verb.to_string()
	} else {
		if verb == "to be" { return "is".to_string() }
		
		let mut c = verb.to_string();
		c.push('s');
		c
	}	
}

// Simple way to make messages like "You are stuck in the web!" vs "The goblin is stuck in the web!"
pub fn format_msg(obj_id: usize, verb: &str, msg: &str, game_obj_db: &mut GameObjectDB) -> String {
	let mut s = String::from("");
	if obj_id == 0 {
		s.push_str("You ");
	} else {
		let m = game_obj_db.npc(obj_id).unwrap();
		s.push_str(&m.npc_name(false));
		s.push(' ');
	}

	s.push_str(&conjugate(obj_id == 0, verb));
	s.push(' ');
	s.push_str(msg);

	s
}

pub trait StringUtils {
	fn capitalize(&self) -> String;
	fn pluralize(&self) -> String;
	fn with_def_article(&self) -> String;
	fn with_indef_article(&self) -> String;
}

// Pre-computed circles of various radiuses
pub const CIRCLE_ROWS_R1: [(i32, i32, i32); 3]  = [(-1, -1, 1), (0, -1, 1), (1, -1, 1)];
pub const CIRCLE_ROWS_R3: [(i32, i32, i32); 7]  = [(-3, -1, 1), (-2, -2, 2), (-1, -3, 3), (0, -3, 3), (1, -3 ,3), (2, -2, 2), (3, -1, 1)];
pub const CIRCLE_ROWS_R5: [(i32, i32, i32); 11] = [(-5, -2, 2), (-4, -3, 3), (-3, -4, 4), (-2, -5, 5), (-1, -5, 5), (0, -5, 5),
											       (1, -5, 5), (2, -5, 5), (3, -4, 4), (4, -3, 3), (5, -2, 2)]; 
pub const CIRCLE_ROWS_R7: [(i32, i32, i32); 15] = [(-7, -2, 2), (-6, -4, 4), (-5, -5, 5), (-4, -6, 6), (-3, -6, 6), (-2, -7, 7),
											       (-1, -7, 7), (0, -7, 7), (1, -7, 7), (2, -7, 7), (3, -6, 6), (4, -6, 6),
											       (5, -5, 5), (6, -4, 4), (7, -2, 2)];
pub const CIRCLE_ROWS_R9: [(i32, i32, i32); 19] = [(-9, -3, 3), (-8, -5, 5), (-7, -6, 6), (-6, -7, 7), (-5, -8, 8), (-4, -8, 8),
											       (-3, -9, 9), (-2, -9, 9), (-1, -9, 9), (0, -9, 9), (1, -9, 9), (2, -9, 9),
											       (3, -9, 9), (4, -8, 8), (5, -8, 8), (6, -7, 7), (7, -6, 6), (8, -5, 5),
											       (9, -3, 3)];											  									

// I started off with this string util stuff as just free-floating functions,
// but I think extending String with a Trait is a bit more rustic?
impl StringUtils for String {
	fn capitalize(&self) -> String {
		// Rust is so intuitive...
		let mut c = self.chars();
		match c.next() {
			None => String::new(),
			Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
		}
	}

	// English is such a mess...
	fn pluralize(&self) -> String {
		// quick n' dirty way of handling turning potion of healing into potions of healing
		if self.contains(" of ") {
			let words: Vec<&str> = self.split(' ').collect();
			let mut result = String::from("");
			result.push_str(words[0]);
			if result.ends_with('s') || result.ends_with('x') || result.ends_with("ch") {
				result.push_str("es");
			} else {
				result.push('s');
			}

			for j in 1..words.len() {
				result.push(' ');
				result.push_str(words[j]);
			}

			result
		} else {
			let mut result = String::from("");
			result.push_str(self);	
			if self.ends_with('s') || self.ends_with('x') || self.ends_with("ch") {
				result.push_str("es");
			} else {
				result.push('s');
			}

			result
		} 		
	}

	fn with_def_article(&self) -> String {
		format!("the {}", self)
	}

	fn with_indef_article(&self) -> String {	
		let first = self.chars().next().unwrap();
		if first == 'a' || first == 'e' || first == 'i' || first == 'o' || first == 'u' || first == 'y' {
			format!("an {}", self)
		} else if first >= '0' && first <= '9' {
			self.to_string()		
		} else {
			format!("a {}", self)			
		}		
	}
}
