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
// Union-find functions to implement disjoint sets
// (handy for finding isolated pockets in maps)
pub fn ds_union(ds: &mut Vec<i32>, r1: i32, r2: i32) {
	ds[r2 as usize] = r1;
	// if ds[r2 as usize] < ds[r1 as usize] {
	// 	ds[r1 as usize] = r2;
	// } else {
	// 	if ds[r1 as usize] == ds[r2 as usize] {
	// 		ds[r1 as usize] -= 1;
	// 	}
	// 	ds[r2 as usize] = r1;
	// }
}

pub fn distance(x1: i32, y1: i32, x2: i32, y2: i32) -> f64 {
	let d = (i32::abs(i32::pow(x1 - x2, 2)) + i32::abs(i32::pow(y1 - y2, 2))) as f64;
	return d.sqrt();
}

pub fn ds_find(ds: &mut Vec<i32>, x: i32) -> i32 {
	if ds[x as usize] < 0 {
		x
	} else {
		ds_find(ds, ds[x as usize])
	}
}

// Straight out of my old scientific computing textbook
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

	// English is a mess and I am pretty sure this isn't this function's
	// final form...
	fn pluralize(&self) -> String {
		let mut result = String::from("");
		result.push_str(self);	
		if self.ends_with("s") || self.ends_with("x") || self.ends_with("ch") {
			result.push_str("es");
		} else {
			result.push_str("s");
		}
		
		result
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
