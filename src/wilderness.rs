// This file is part of RogueVillage, a roguelike game.
//
// RogueVillage is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// YarrL is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with RogueVillage.  If not, see <https://www.gnu.org/licenses/>.

use std::collections::HashMap;
use std::fs;

use rand::thread_rng;
use rand::Rng;

use super::Map;

use crate::map;
use crate::map::Tile;

pub fn test_map() -> Map {
	let mut map: Map = HashMap::new();

	let contents = fs::read_to_string("wilderness.txt")
								.expect("Unable to find test wilderness file!"); 	

	let mut row = 0;								
	for line in contents.split('\n') {
		let mut col = 0;		
		for ch in line.chars() {
			let tile = match ch {
				'^' => Tile::Mountain,
				'T' => Tile::Tree,
				'.' => Tile::StoneFloor,
				'`' => Tile::Grass,
				'~' => Tile::DeepWater,
				'#' => Tile::Wall,
				'+' => Tile::Door(false),
				'-' => Tile::Window(ch),
				'|' => Tile::Window(ch),
				_ => Tile::Lava, // This shouldn't actually happen...
			};
			map.insert((row, col, 0), tile);
			col += 1;
		}
		row += 1;
	}

	map
}
