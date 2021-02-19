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

use crate::dungeon;
use crate::map::Tile;
use crate::wilderness;

pub fn generate_world() -> Map {
    let mut map = wilderness::test_map();
    let dungeon_entrance = (234, 216);

    let dungeon_width = 125;
    let dungeon_height = 40;
    let mut dungeon_level = dungeon::draw_level(125, 40);
    let mut floors = Vec::new();
    for i in 0..dungeon_level.len() {
        if dungeon_level[i] == Tile::StoneFloor {
            floors.push(i);
        }
    }
    let stairs_loc = floors[thread_rng().gen_range(0, floors.len())];
    dungeon_level[stairs_loc] = Tile::StairsUp;
    let stairs_row = stairs_loc / dungeon_width;
    let stairs_col = stairs_loc - (stairs_row * dungeon_width);
    for r in 0..dungeon_height {
        for c in 0..dungeon_width {
            let i = r * dungeon_width + c;
            let curr_row = dungeon_entrance.0 - stairs_row + r;
            let curr_col = dungeon_entrance.1 - stairs_col + c;
            map.insert((curr_row as i32, curr_col as i32, 1), dungeon_level[i]);
        }
    }
    println!("{} {} {:?}", stairs_row, stairs_col, dungeon_level[stairs_row * dungeon_width + stairs_col]);
    //let delta_row = dungeon_entrance.0 - stairs_loc
    //dungeon_level[stairs_loc] = map::Tile::Portal;

    map.insert((dungeon_entrance.0 as i32, dungeon_entrance.1 as i32, 0), Tile::Portal);

    map
}