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

use std::collections::{HashMap, HashSet};
use rand::thread_rng;
use rand::Rng;

use super::{Map, NPCTable};

use crate::actor::{Actor, Mayor, SimpleMonster};
use crate::display::{BRIGHT_RED};
use crate::dungeon;
use crate::map::Tile;
use crate::pathfinding::find_path;
use crate::wilderness;

pub struct Fact {
    pub detail: String,
    pub timestamp: i32,
    pub location: (i32, i32, i8),
}

impl Fact {
    pub fn new(detail: String, timestamp: i32, location: (i32, i32, i8)) -> Fact {
        Fact { detail, timestamp, location }
    }
}

pub struct WorldInfo {
    pub facts: Vec<Fact>,
    pub town_boundary: (i32, i32, i32, i32),
    pub town_name: String,
    pub town_square: HashSet<(i32, i32, i8)>,
}

impl WorldInfo {
    pub fn new(town_name: String, town_boundary: (i32, i32, i32, i32)) -> WorldInfo {
        WorldInfo { town_name, facts: Vec::new(), town_boundary, town_square: HashSet::new() }
    }
}

// Draw paths in town. Once I've trqnslated the town generation code from Python 
// to rust and am making a new town eah game, this should be moved to that code
// (And I'll be calculating the townsquare then anyhow)
fn draw_paths_in_town(map: &mut Map, town_square: &HashSet<(i32, i32, i8)>) {
    let mut doors = HashSet::new();

    let adj: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    for r in 100..138 {
        for c in 45..112 {
            let loc = (r, c, 0);
            if let Tile::Door(_) = map[&loc] {
                // Draw dirt outside each door
                for a in adj.iter() {
                    let t = map[&(loc.0 + a.0, loc.1 + a.1, 0)];
                    if t == Tile::Grass || t == Tile::Tree {
                        map.insert((loc.0 + a.0, loc.1 + a.1, 0), Tile::Dirt);
                    }
                }
                doors.insert(loc);
            }
        }
    }

    // pick random spot in the town square for paths to converge on
    let mut passable = HashMap::new();
    passable.insert(Tile::Grass, 1.0);
    passable.insert(Tile::Dirt, 1.0);
    passable.insert(Tile::Bridge, 1.0);
    passable.insert(Tile::Water, 3.0);
    passable.insert(Tile::DeepWater, 3.0);
    let j = thread_rng().gen_range(0, town_square.len());
    let centre = town_square.iter().nth(j).unwrap();
    let mut path = Vec::new();
    for door in doors {
        path = find_path(map, false, door.0, door.1, 0, centre.0, centre.1, 150, &passable);
        if (path.len() > 0) {
            //path.pop();
            for sq in path {
                let loc = (sq.0, sq.1, 0);
                if let Tile::Grass = map[&loc] {
                    map.insert(loc, Tile::Dirt);
                } else if let Tile::DeepWater = map[&loc] {
                    map.insert(loc, Tile::Bridge);
                    let mut col = loc.1 + 1;
                    while map[&(loc.0, col, loc.2)] == Tile::DeepWater {
                         map.insert((loc.0, col, loc.2), Tile::Bridge);
                         col += 1;
                    }
                    let mut col = loc.1 - 1;
                    while map[&(loc.0, col, loc.2)] == Tile::DeepWater {
                         map.insert((loc.0, col, loc.2), Tile::Bridge);
                         col -= 1;
                    }
                }
            }
        }
    }
}

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

pub fn find_all_valleys(map: &Map) -> Vec<HashSet<(i32, i32, i8)>> {
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

    valleys
}

fn count_adj_mountains(map: &Map, loc: (i32, i32, i8)) -> u32 {
    let mut adj = 0;
    for r in -1..2 {
        for c in -1..2 {
            let nl = (loc.0 + r, loc.1 + c, loc.2);
            if map.contains_key(&nl) && (map[&nl] == Tile::Mountain || map[&nl] == Tile::SnowPeak) {
                adj += 1;
            }
        }
    }

    adj
}

// We want the entrance to the main dungeon to be nicely nestled into the mountains so we'll look
// for locations that are surround by at least 4 mountains
fn find_good_dungeon_entrance(map: &Map, sqs: &HashSet<(i32, i32, i8)>) -> (i32, i32, i8) {
    let mut options = Vec::new();

    for loc in sqs {
        if count_adj_mountains(map, *loc) >= 4 {
            options.push(loc);
        }
    }

    let j = thread_rng().gen_range(0, options.len());
    *options[j]
}

pub fn generate_world() -> (Map, WorldInfo, NPCTable) {
    let mut map = wilderness::test_map();
    let valleys = find_all_valleys(&map);
    // We want to place the dungeon entrance somewhere in the largest 'valley', which will be
    // the main section of the overworld

    // tbh, I start searching for valleys at 0, 0 so valley[0] will always be the main one
    let mut max = 0;
    let mut max_id = 0;
    for v in 0..valleys.len() {
        if valleys[v].len() > max {
            max = valleys[v].len();
            max_id = v;
        }
    }

    let dungeon_entrance = find_good_dungeon_entrance(&map, &valleys[max_id]);
    let town_name = "Skara Brae";

    let mut world_info = WorldInfo::new(town_name.to_string(),(100, 100, 135, 110));
    world_info.facts.push(Fact::new("dungeon location".to_string(), 0, dungeon_entrance));
    world_info.facts.push(Fact::new("town name is Skara Brae".to_string(), 0, (0, 0, 0)));
    world_info.town_square = HashSet::new();
    for r in 118..123 {
        for c in 65..85 {
            if map[&(r, c, 0)].is_passable() && map[&(r, c, 0)] != Tile::DeepWater {
                world_info.town_square.insert((r, c, 0));
            }
        }
    }

    draw_paths_in_town(&mut map, &world_info.town_square);

    // Assuming in the future we've generated a fresh town and now want to add in townsfolk
    let mut mayor = Mayor::new("Quimby".to_string(), (120, 79, 0));
    mayor.home.insert((115, 104, 0));
    mayor.home.insert((115, 105, 0));
    mayor.home.insert((116, 104, 0));
    mayor.home.insert((116, 105, 0));
    mayor.home.insert((117, 104, 0));
    mayor.home.insert((117, 105, 0));
    mayor.home.insert((118, 101, 0));
    mayor.home.insert((118, 102, 0));
    mayor.home.insert((118, 103, 0));
    mayor.home.insert((118, 104, 0));
    mayor.home.insert((118, 105, 0));
    mayor.home.insert((119, 101, 0));
    mayor.home.insert((119, 102, 0));
    mayor.home.insert((119, 103, 0));
    mayor.home.insert((119, 104, 0));
    mayor.home.insert((119, 105, 0));
    mayor.home.insert((120, 100, 0));
    mayor.home.insert((120, 101, 0));
    mayor.home.insert((120, 102, 0));
    mayor.home.insert((120, 103, 0));
    mayor.home.insert((120, 104, 0));
    mayor.home.insert((120, 105, 0));
    mayor.facts_known.push(0);
    mayor.facts_known.push(1);
    
    let mut npcs: NPCTable = HashMap::new();
    npcs.insert(mayor.get_loc(), Box::new(mayor));
    let g1 = SimpleMonster::new("goblin".to_string(), (140, 140, 0), 'o', BRIGHT_RED);
    npcs.insert(g1.get_loc(), Box::new(g1));
    
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
    let stairs_row_delta = dungeon_entrance.0 - stairs_row as i32;
    let stairs_col_delta = dungeon_entrance.1 - stairs_col as i32;
    for r in 0..dungeon_height {
        for c in 0..dungeon_width {
            let i = r * dungeon_width + c;
            let curr_row = stairs_row_delta + r as i32;
            let curr_col = stairs_col_delta + c as i32;
                        
            map.insert((curr_row, curr_col, 1), dungeon_level[i]);
        }
    }
    
    map.insert((dungeon_entrance.0 as i32, dungeon_entrance.1 as i32, 0), Tile::Portal);
    
    (map, world_info, npcs)
}