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

extern crate serde;

use std::collections::{HashMap, HashSet};
use std::time::Instant;
use rand::{prelude::{IteratorRandom, SliceRandom}, thread_rng};
use rand::Rng;
use serde::{Serialize, Deserialize};

use super::{EventType, GameObjects, Map};

use crate::dungeon;
use crate::dungeon::Vault;
use crate::game_obj::GameObject;
use crate::items::{GoldPile, Item};
use crate::map::{ShrineType, SpecialSquare, Tile, TriggerType};
use crate::town;
use crate::town::TownBuildings;
use crate::pathfinding;
use crate::util;
use crate::wilderness;

pub const WILDERNESS_SIZE: usize = 257;

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct WorldInfo {
    pub facts: Vec<Fact>,
    pub town_boundary: (i32, i32, i32, i32),
    pub town_name: String,
    pub town_square: HashSet<(i32, i32, i8)>,
    pub tavern_name: String,
    pub town_buildings: Option<TownBuildings>,
}

impl WorldInfo {
    pub fn new(town_name: String, town_boundary: (i32, i32, i32, i32), tavern_name: String) -> WorldInfo {
        WorldInfo { town_name, facts: Vec::new(), town_boundary, town_square: HashSet::new(),
            tavern_name, town_buildings: None }
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

// Add an old road leading away from the dungeon that eventually trails off
// I think this sometimes hits an infinite loop looking for a good path for the 
// trail, which suggests there's probably some degenerate cases for placing the
// dungeon are actually impassable. But I have to debug that a bunch as it 
// sometimes places the entrance in bad places like a mountain river with no
// ground access
fn add_old_road(map: &mut Map, start: (i32, i32, i8)) {
    let mut rng = rand::thread_rng();
    let mut passable = HashMap::new();
    passable.insert(Tile::Grass, 1.0);
    passable.insert(Tile::Dirt, 1.0);
    passable.insert(Tile::Tree, 1.0);
    passable.insert(Tile::StoneFloor, 1.0);
    passable.insert(Tile::DeepWater, 1.0);

    loop {
        let row = start.0 - rng.gen_range(10, 20);
        let col = rng.gen_range(start.1 - 15, start.1 + 15);
        
        if !map.contains_key(&(row, col, 0)) || !map[&(row, col, 0)].passable() { continue; }
        
        let path = pathfinding::find_path(map, false, start.0, start.1, 0, row, col, 40, &passable);
        let mut draw_sq = 1.0;
        if path.len() > 0 {
            for sq in path {
                if map[&(sq.0, sq.1, 0)] != Tile::DeepWater {
                    if rng.gen_range(0.0, 1.0) < draw_sq {
                        map.insert((sq.0, sq.1, 0), Tile::StoneFloor);
                        draw_sq -= 0.05;
                    }
                }
            }
            break;
        }        
    }
}

fn random_sq<T: Copy>(sqs: &HashSet<T>) -> T {
    // I can't believe this is the easiest way I've found to pick a random element
    // from a HashSet T_T
    //
    // In C# you can just access HashSets by index :/
    let mut rng = rand::thread_rng();
    let items = sqs.iter()
                 .map(|i| *i)
                 .collect::<Vec<T>>();
    
    let item = items.choose(&mut rng).unwrap();

    item.clone()
}

fn set_stairs(dungeon: &mut Vec<Vec<Tile>>, width: usize, height: usize) -> (usize, usize) {
    let mut rng = rand::thread_rng();
    let mut open_sqs = Vec::new();
    for level_id in 0..dungeon.len() {
        let mut open = HashSet::new();
        for r in 0..height {
            for c in 0..width {
                if dungeon[level_id][r * width + c] == Tile::StoneFloor {
                    open.insert((r, c));
                }
            }
        }
        open_sqs.push(open);
    }

    // First find the up stairs on level 1 of the dungeon, which is the entrance. Just grab
    // any ol' open square 
    let entrance = random_sq(&open_sqs[0]).clone();
    dungeon[0][entrance.0 * width + entrance.1] = Tile::StairsUp;
    open_sqs[0].remove(&entrance);

    // I wanted the levels of my dungeon to be aligned. (Ie., if the stairs down from level 3 are at 4,16 then
    // the stairs back up on level 4 will be at 4,16 as well)
    for n in 0..dungeon.len() - 1 {
        let options = open_sqs[n].intersection(&open_sqs[n + 1]);
        let stairs = options.choose(&mut rng).unwrap().clone();
        dungeon[n][stairs.0 * width + stairs.1] = Tile::StairsDown;
        dungeon[n + 1][stairs.0 * width + stairs.1] = Tile::StairsUp;
        open_sqs[n].remove(&stairs);
        open_sqs[n + 1].remove(&stairs);        
    }

    entrance
}

fn random_open_adj(open: &HashSet<(i32, i32, i8)>, loc: (i32, i32, i8)) -> Option<(i32, i32, i8)> {
    let mut rng = rand::thread_rng();
    //let mut options = Vec::new();

    let options = util::ADJ.iter()
                           .map(|d| (loc.0 + d.0, loc.1 + d.1, loc.2))
                           .filter(|adj| open.contains(&adj))
                           .collect::<Vec<(i32, i32, i8)>>();

    if options.len() > 0 {
        Some(options[rng.gen_range(0, options.len())])
    } else {
        None
    }
}

fn add_fire_pit(level: usize, map: &mut Map, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>, game_objs: &mut GameObjects) {
    let mut rng = rand::thread_rng();
    let loc = random_sq(&floor_sqs[&(level - 1)]);
    map.insert(loc, Tile::OldFirePit(rng.gen_range(0, 5)));
    floor_sqs.get_mut(&(level -1))
             .unwrap()
             .remove(&loc);
    if let Some(adj) = random_open_adj(&floor_sqs[&(level - 1)], loc) {
        let mut note = Item::get_item(game_objs, "note").unwrap();
        note.text = Some(("burnt scrap".to_string(), "Is there no end to the swarms of kobolds?".to_string()));
        note.location = adj;
        game_objs.add(Box::new(note));
    }
    let amt = rng.gen_range(4, 11);
    let mut pile = GoldPile::new(game_objs.next_id(), amt, loc);
    pile.hide();
    game_objs.add(Box::new(pile));
}

fn add_shrine(world_info: &mut WorldInfo, level: usize, map: &mut Map, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>, game_objs: &mut GameObjects) {
    let mut rng = rand::thread_rng();
    let loc = random_sq(&floor_sqs[&(level - 1)]);
    let special_sq = SpecialSquare::new(game_objs.next_id(), Tile::Shrine(ShrineType::Woden), loc, true, 3);
    game_objs.listeners.insert((special_sq.get_object_id(), EventType::EndOfTurn));
    game_objs.add(Box::new(special_sq));

    map.insert(loc, Tile::Shrine(ShrineType::Woden));
    floor_sqs.get_mut(&(level - 1))
            .unwrap()
            .remove(&loc);
    let fact = Fact::new(String::from("shrine to woden"), 0, loc);
    world_info.facts.push(fact);
    println!("{:?}", world_info.facts);
}

fn decorate_levels(world_info: &mut WorldInfo, map: &mut Map, deepest_level: i8, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>,
            game_objs: &mut GameObjects) {
    let mut rng = rand::thread_rng();
    let mut curr_level = deepest_level;
    while curr_level > 0 {
        if curr_level < 3 {
            add_fire_pit(curr_level as usize, map, floor_sqs, game_objs)             
        }

        if curr_level == 3 {
            add_shrine(world_info, curr_level as usize, map, floor_sqs, game_objs)
        }
        curr_level -= 1;
    }
}

fn build_dungeon(world_info: &mut WorldInfo, map: &mut Map, entrance: (i32, i32, i8), game_objs: &mut GameObjects) {
    let width = 125;
    let height = 40;
    let mut floor_sqs = HashMap::new();
    let max_level = 1;
        
    let mut dungeon = Vec::new();
    for n in 0..max_level {
        let result = dungeon::draw_level(width, height);
        let level = result.0;
        // vaults are rooms with only one entrance, which are useful for setting puzzles
        let vaults = result.1;
        println!("{:?}", vaults);

        dungeon.push(level);
        floor_sqs.insert(n, HashSet::new());
    }

    let stairs = set_stairs(&mut dungeon, width, height);
    // Copy the dungeon onto the world map
    for lvl in 0..max_level {
        let stairs_row_delta = entrance.0 - stairs.0 as i32;
        let stairs_col_delta = entrance.1 - stairs.1 as i32;
        for r in 0..height {
            for c in 0..width {
                let i = r * width + c;
                let curr_row = r as i32 + stairs_row_delta;
                let curr_col = c as i32 + stairs_col_delta;
                map.insert((curr_row, curr_col, lvl as i8 + 1), dungeon[lvl][i]);
                
                if dungeon[lvl][i] == Tile::StoneFloor {
                    let fqs = floor_sqs.get_mut(&lvl).unwrap();
                    fqs.insert((curr_row, curr_col, lvl as i8 + 1));
                }
            }
        }
    }

    let trigger = Tile::Trigger(TriggerType::Step);
    map.insert((entrance.0 - 1, entrance.1, 1), trigger);
    let special_sq = SpecialSquare::new(game_objs.next_id(), Tile::Trigger(TriggerType::Step), (entrance.0 - 1, entrance.1, 1), false, 0);
    game_objs.listeners.insert((special_sq.get_object_id(), EventType::SteppedOn));
    game_objs.add(Box::new(special_sq));

    decorate_levels(world_info, map, max_level as i8, &mut floor_sqs, game_objs)
}

pub fn generate_world(game_objs: &mut GameObjects) -> (Map, WorldInfo) {
    let map_start = Instant::now();
    let mut map = wilderness::gen_wilderness_map();
    let map_end = map_start.elapsed();
    println!("Time to make world map: {:?}", map_end);

    let mut world_info = town::create_town(&mut map, game_objs);

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
    
    let dungeon_start = Instant::now();
    build_dungeon(&mut world_info, &mut map, dungeon_entrance, game_objs);
    let dungeon_end = dungeon_start.elapsed();
    println!("Time to make dungeon: {:?}", dungeon_end);

    world_info.facts.push(Fact::new("dungeon location".to_string(), 0, dungeon_entrance));

    add_old_road(&mut map, dungeon_entrance);
    map.insert((dungeon_entrance.0 as i32, dungeon_entrance.1 as i32, 0), Tile::Portal);
    
    (map, world_info)
}