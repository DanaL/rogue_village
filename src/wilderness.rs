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

use rand::thread_rng;
use rand::Rng;

use super::Map;

use crate::map;
use crate::map::Tile;

pub fn test_map() -> Map {
	let mut map: Map = HashMap::new();

	for col in 0..100 {
		map.insert((0, col, 0), Tile::WorldEdge);
		map.insert((99, col, 0), Tile::WorldEdge);
	}
	for row in 1..99 {
		map.insert((row, 0, 0), Tile::WorldEdge);
		map.insert((row, 99, 0), Tile::WorldEdge);
	}

	let mut rng = thread_rng();
	for col in 1..99 {
		let water: u16 = rng.gen_range(20, 50);
		let ground: u16 = rng.gen_range(10, 30);
		
		for row in 1..water {
			map.insert((row, col, 0), Tile::DeepWater);
		}
		for row in water..water+ground {
			let x = rng.gen_range(0, 3);
			if x == 0 {
				map.insert((row, col, 0), Tile::Dirt);
			} else if x == 1 {
				map.insert((row, col, 0), Tile::Grass);
			} else {
				map.insert((row, col, 0), Tile::Tree);
			}
		}
		for row in water+ground..99 {
			map.insert((row, col, 0), Tile::Mountain);
		}
	}

	map
}
