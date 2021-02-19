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

use std::collections::{HashMap, HashSet};
use rand::thread_rng;
use rand::Rng;

use super::Map;

use crate::dungeon;
use crate::map::Tile;
use crate::util;
use crate::wilderness;

// The random wilderness generator will inevitably create pockets of
// traversable land complately surrounded by mountains. I don't want to 
// stick the main dungeon in one of those, and they might also be useful
// for hidden secrets later on.
// I could/should generalize this so it takes a set of tiles to ignore and
// then I can search for any kind of distinct pockets on any level
fn find_valley(map: &Map, start_loc: (i32, i32, i8)) -> HashSet<(i32, i32, i8)> {
    let mut queue = vec![start_loc];
    let mut visited = HashSet::new();

    while queue.len() > 0 {
        let loc = queue.pop().unwrap();
        visited.insert(loc);

        // I'm going to only consider adjacent NESW squares in case I later decide
        // diaganol movement isn't a thing 
        let nl = (loc.0 - 1, loc.1, loc.2);
        if !visited.contains(&nl) && map.contains_key(&nl) {
            if map[&nl] != Tile::Mountain && map[&nl] != Tile::SnowPeak { 
                queue.push(nl); 
            }
        }
        let nl = (loc.0 + 1, loc.1, loc.2);
        if !visited.contains(&nl) && map.contains_key(&nl) {
            if map[&nl] != Tile::Mountain && map[&nl] != Tile::SnowPeak { 
                queue.push(nl); 
            }
        }
        let nl = (loc.0, loc.1 - 1, loc.2);
        if !visited.contains(&nl) && map.contains_key(&nl) {
            if map[&nl] != Tile::Mountain && map[&nl] != Tile::SnowPeak { 
                queue.push(nl); 
            }
        }
        let nl = (loc.0, loc.1 + 1, loc.2);
        if !visited.contains(&nl) && map.contains_key(&nl) {
            if map[&nl] != Tile::Mountain && map[&nl] != Tile::SnowPeak { 
                queue.push(nl); 
            }
        }
    }
    
    visited
}

pub fn find_lost_valleys(map: &Map, width: i32) {
    let mut valleys = vec![find_valley(map, (0, 0, 0))];

    for loc in map.keys() {
        if loc.2 != 0 || map[&loc] == Tile::Mountain || map[&loc] == Tile::SnowPeak {
            continue;
        }

        let mut already_found = false;
        for valley in &valleys {
            if valley.contains(loc) {
                already_found = true;
                break;
            }            
        }
        
        if !already_found {
            let valley = find_valley(map, *loc);
            valleys.push(valley);
        }
    }

    for valley in valleys {
        println!("Size of valley: {}", valley.len());
    }
}

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
    
    map.insert((dungeon_entrance.0 as i32, dungeon_entrance.1 as i32, 0), Tile::Portal);

    map
}