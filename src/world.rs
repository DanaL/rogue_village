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
use rand::{prelude::{IteratorRandom}, thread_rng};
use rand::Rng;
use serde::{Serialize, Deserialize};

use super::{EventType, GameObjects, Map};

use crate::actor::MonsterFactory;
use crate::dungeon;
use crate::dungeon::Vault;
use crate::items::{GoldPile, Item};
use crate::map::{DoorState, ShrineType, SpecialSquare, Tile};
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

    while !queue.is_empty() {
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
        if !path.is_empty() {
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
    let items = sqs.iter().copied();
                
    let item = items.choose(&mut rng).unwrap();

    item.clone()
}

fn set_stairs(dungeon: &mut Vec<Vec<Tile>>, width: usize, height: usize) -> (usize, usize) {
    let mut rng = rand::thread_rng();
    let mut open_sqs = Vec::new();
    for (_, level) in dungeon.iter().enumerate() {
        let mut open = HashSet::new();
        for r in 0..height {
            for c in 0..width {
                if level[r * width + c] == Tile::StoneFloor {
                    open.insert((r, c));
                }
            }
        }
        open_sqs.push(open);
    }

    // First find the up stairs on level 1 of the dungeon, which is the entrance. Just grab
    // any ol' open square 
    let entrance = random_sq(&open_sqs[0]);
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
        note.item.as_mut().unwrap().text = Some(("burnt scrap".to_string(), "Is there no end to the swarms of kobolds?".to_string()));
        note.location = adj;
        game_objs.add(note);
    }
    let amt = rng.gen_range(4, 11);
    let mut pile = GoldPile::make(game_objs, amt, loc);
    pile.hide();
    game_objs.add(pile);
}

fn add_shrine(world_info: &mut WorldInfo, level: usize, map: &mut Map, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>, game_objs: &mut GameObjects) {
    let loc = random_sq(&floor_sqs[&(level - 1)]);
    let shrine = SpecialSquare::make(Tile::Shrine(ShrineType::Woden), loc, true, 3, game_objs);
    game_objs.listeners.insert((shrine.object_id, EventType::EndOfTurn));
    game_objs.add(shrine);

    map.insert(loc, Tile::Shrine(ShrineType::Woden));
    floor_sqs.get_mut(&(level - 1))
            .unwrap()
            .remove(&loc);
    let fact = Fact::new(String::from("shrine to woden"), 0, loc);
    world_info.facts.push(fact);
}

fn loc_in_vault(vault: &Vault, loc: (i32, i32, i8)) -> bool {
    loc.0 >= vault.r1 && loc.0 <= vault.r2 && loc.1 >= vault.c1 && loc.1 <= vault.c2
}

fn place_simple_triggered_gate(_world_info: &mut WorldInfo, map: &mut Map, game_objs: &mut GameObjects, trigger_loc: (i32, i32, i8),
        vault_loc: (i32, i32, i8)) {
    map.insert(trigger_loc, Tile::Trigger);
    map.insert(vault_loc, Tile::Gate(DoorState::Closed));
    let gate = SpecialSquare::make(Tile::Gate(DoorState::Closed), vault_loc, true, 0, game_objs);
    let gate_id = gate.object_id;
    game_objs.add(gate);

    let mut trigger = SpecialSquare::make(Tile::Trigger, trigger_loc, false, 0, game_objs);
    let trigger_id = trigger.get_object_id();
    trigger.special_sq.as_mut().unwrap().target = Some(gate_id);
    game_objs.add(trigger);
    game_objs.listeners.insert((trigger_id, EventType::SteppedOn));
}

fn simple_triggered_gate(world_info: &mut WorldInfo, map: &mut Map, floors: &mut HashSet<(i32, i32, i8)>,
        game_objs: &mut GameObjects, vault: &Vault, level: i8) {
    // Find a place for the trigger. Probably need a bail out option after X iterations in case it's some
    // weird dungeon layout where a trigger can't be placed.
    let mut delta = 2;
    loop {
        let loc = (vault.entrance.0 + delta, vault.entrance.1, level);
        let vault_entrance = (vault.entrance.0, vault.entrance.1, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_objs, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        let loc = (vault.entrance.0 - delta, vault.entrance.1, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_objs, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        let loc = (vault.entrance.0, vault.entrance.1 + delta, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_objs, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        let loc = (vault.entrance.0, vault.entrance.1 - delta, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_objs, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        delta += 1;
    }
}

fn light_triggered_gate(_world_info: &mut WorldInfo, map: &mut Map, floors: &mut HashSet<(i32, i32, i8)>,
        game_objs: &mut GameObjects, vault: &Vault, level: i8) {
    let vault_loc = (vault.entrance.0, vault.entrance.1, level);
    map.insert(vault_loc, Tile::Gate(DoorState::Closed));
    let gate = SpecialSquare::make(Tile::Gate(DoorState::Closed), vault_loc, true, 0, game_objs);
    let gate_id = gate.object_id;
    game_objs.add(gate);
    game_objs.listeners.insert((gate_id, EventType::LitUp));
    floors.remove(&vault_loc);
}

fn add_vault(world_info: &mut WorldInfo, map: &mut Map, floors: &mut HashSet<(i32, i32, i8)>,
            game_objs: &mut GameObjects, vaults: &Vec<Vault>, level: i8) {
    // In the real game, I want to make sure I never create a gated vault in a room with the upstairs 
    // because that would result in a dungeon where the player probably can't progress without magic
    let mut rng = rand::thread_rng();
    let vault_num = rng.gen_range(0, vaults.len());
    let vault = &vaults[vault_num];
    
    //simple_triggered_gate(world_info, map, floors, game_objs, vault, level);
    light_triggered_gate(world_info, map, floors, game_objs, vault, level);
}

fn decorate_levels(world_info: &mut WorldInfo, map: &mut Map, deepest_level: i8, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>,
            game_objs: &mut GameObjects, vaults: HashMap<usize, Vec<Vault>>) {
    //let mut rng = rand::thread_rng();
    let mut curr_level = deepest_level;
    while curr_level > 0 {
        if curr_level < 3 {
            add_fire_pit(curr_level as usize, map, floor_sqs, game_objs)             
        }

        if curr_level == 1 {
            add_shrine(world_info, curr_level as usize, map, floor_sqs, game_objs)
        }

        if !vaults[&(curr_level as usize - 1)].is_empty() {
            let floors = floor_sqs.get_mut(&(curr_level as usize - 1)).unwrap();
            add_vault(world_info, map, floors, game_objs, &vaults[&(curr_level as usize - 1)], curr_level);
        }

        curr_level -= 1;
    }
}

fn populate_levels(_world_info: &mut WorldInfo, deepest_level: i8, floor_sqs: &HashMap<usize, HashSet<(i32, i32, i8)>>,
            game_objs: &mut GameObjects, monster_fac: &MonsterFactory) {
    let mut curr_level = deepest_level;
    let mut rng = rand::thread_rng();
    while curr_level > 0 {
        let level_index = curr_level as usize - 1;

        for _ in 0..10 {
            let loc = random_sq(&floor_sqs[&level_index]);
            if rng.gen_range(0.0, 1.0) < 0.5 {
                monster_fac.add_monster("kobold", loc, game_objs);
            } else {
                monster_fac.add_monster("goblin", loc, game_objs);
            }
        }
        curr_level -= 1;
    }
}

fn build_dungeon(world_info: &mut WorldInfo, map: &mut Map, entrance: (i32, i32, i8), game_objs: &mut GameObjects, monster_fac: &MonsterFactory) {
    let width = 125;
    let height = 40;
    let mut floor_sqs = HashMap::new();
    let mut vaults = HashMap::new();
    let max_level = 1;
        
    let mut dungeon = Vec::new();
    for n in 0..max_level {
        let result = dungeon::draw_level(width, height);
        let level = result.0;
        
        dungeon.push(level);
        floor_sqs.insert(n, HashSet::new());
        vaults.insert(n, result.1); // vaults are rooms with only one entrance, which are useful for setting puzzles        
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

        // Need to update the co-ordinates in the vaults
        let curr_vaults = vaults.get_mut(&lvl).unwrap();
        for vault in curr_vaults {
            vault.r1 += stairs_row_delta;
            vault.c1 += stairs_col_delta;
            vault.r2 += stairs_row_delta;
            vault.c2 += stairs_col_delta;
            vault.entrance = (vault.entrance.0 + stairs_row_delta, vault.entrance.1 + stairs_col_delta);
        }
    }

    decorate_levels(world_info, map, max_level as i8, &mut floor_sqs, game_objs, vaults);
    populate_levels(world_info, max_level as i8, &floor_sqs, game_objs, monster_fac);
}

pub fn generate_world(game_objs: &mut GameObjects, monster_fac: &MonsterFactory) -> (Map, WorldInfo) {
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
    for (n, valley) in valleys.iter().enumerate() {
        if valley.len() > max {
            max = valley.len();
            max_id = n;
        }
    }

    let dungeon_entrance = find_good_dungeon_entrance(&map, &valleys[max_id]);
    
    let dungeon_start = Instant::now();
    build_dungeon(&mut world_info, &mut map, dungeon_entrance, game_objs, monster_fac);
    let dungeon_end = dungeon_start.elapsed();
    println!("Time to make dungeon: {:?}", dungeon_end);

    world_info.facts.push(Fact::new("dungeon location".to_string(), 0, dungeon_entrance));

    add_old_road(&mut map, dungeon_entrance);
    map.insert((dungeon_entrance.0 as i32, dungeon_entrance.1 as i32, 0), Tile::Portal);
    
    (map, world_info)
}