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


// Some miscellaneous strucs and functions used in a few plces

use std::f32;
use std::fs;

use rand::Rng;

//use crate::dice::roll;
//use crate::items::Item;
use crate::map;
use super::GameState;

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

pub fn ds_find(ds: &mut Vec<i32>, x: i32) -> i32 {
	if ds[x as usize] < 0 {
		x
	} else {
		ds_find(ds, ds[x as usize])
	}
}

pub fn capitalize_word(word: &str) -> String {
	// Rust is so intuitive...
	let mut c = word.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
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
