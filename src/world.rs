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

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;
use rand::{prelude::{IteratorRandom}, thread_rng};
use rand::Rng;
use serde::{Serialize, Deserialize};

use super::{EventType, Map};

use crate::npc::MonsterFactory;
use crate::dungeon;
use crate::dungeon::Vault;
use crate::game_obj::{GameObject, GameObjects, GameObjectDB};
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
    pub player_name: String,
}

impl WorldInfo {
    pub fn new(town_name: String, town_boundary: (i32, i32, i32, i32), tavern_name: String) -> WorldInfo {
        WorldInfo { town_name, facts: Vec::new(), town_boundary, town_square: HashSet::new(),
            tavern_name, town_buildings: None, player_name: "".to_string() }
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

fn check_entrance_candidate(map: &Map, loc: (i32, i32, i8)) -> (u8, u8) {
    let mut adj_mountains = 0;
    let mut adj_water = 0;
    for r in -1..2 {
        for c in -1..2 {
            let nl = (loc.0 + r, loc.1 + c, loc.2);
            if map.contains_key(&nl) {
                if map[&nl] == Tile::Mountain || map[&nl] == Tile::SnowPeak {
                    adj_mountains += 1;
                }
                if map[&nl] == Tile::DeepWater {
                    adj_water += 1;
                }
            }
        }
    }

    (adj_mountains, adj_water)
}

// We want the entrance to the main dungeon to be nicely nestled into the mountains so we'll look
// for locations that are surround by at least 4 mountains
fn find_good_dungeon_entrance(map: &Map, sqs: &HashSet<(i32, i32, i8)>) -> (i32, i32, i8) {
    let mut options = Vec::new();

    for loc in sqs {
        let (adj_mountains, adj_water) = check_entrance_candidate(map, *loc);
        if adj_mountains + adj_water > 7 {
            continue;
        }

        if adj_mountains >= 4 {
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
        
        let path = pathfinding::find_path(map, None, false, start.0, start.1, 0, row, col, 40, &passable);
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
                
    items.choose(&mut rng).unwrap()
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

    if !options.is_empty() {
        Some(options[rng.gen_range(0, options.len())])
    } else {
        None
    }
}

fn add_fire_pit(level: usize, map: &mut Map, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>, game_obj_db: &mut GameObjectDB) {
    let mut rng = rand::thread_rng();
    let loc = random_sq(&floor_sqs[&(level - 1)]);
    map.insert(loc, Tile::OldFirePit(rng.gen_range(0, 5)));
    floor_sqs.get_mut(&(level -1))
             .unwrap()
             .remove(&loc);
    if let Some(adj) = random_open_adj(&floor_sqs[&(level - 1)], loc) {
        let mut note = Item::get_item(game_obj_db, "note").unwrap();
        note.set_loc(adj);
        if let GameObjects::Item(item) = &mut note {
            item.text = Some(("burnt scrap".to_string(), "Is there no end to the swarms of kobolds?".to_string()));            
        }
        game_obj_db.add(note);
    }
    let amt = rng.gen_range(4, 11);
    let mut pile = GoldPile::make(game_obj_db, amt, loc);
    pile.hide();
    game_obj_db.add(pile);
}

fn add_teleport_trap(level: usize, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>, game_obj_db: &mut GameObjectDB) {
    let loc = random_sq(&floor_sqs[&(level - 1)]);
    let trap = SpecialSquare::teleport_trap(loc, game_obj_db);
    game_obj_db.listeners.insert((trap.obj_id(), EventType::SteppedOn));
    game_obj_db.add(trap);
    println!("Trap loc: {:?}", loc);
    floor_sqs.get_mut(&(level - 1))
            .unwrap()
            .remove(&loc);
}

fn add_shrine(world_info: &mut WorldInfo, level: usize, map: &mut Map, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>, game_obj_db: &mut GameObjectDB) {
    let loc = random_sq(&floor_sqs[&(level - 1)]);
    let shrine = SpecialSquare::make(Tile::Shrine(ShrineType::Woden), loc, true, 3, game_obj_db);
    game_obj_db.listeners.insert((shrine.obj_id(), EventType::Update));
    game_obj_db.add(shrine);

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

fn place_simple_triggered_gate(_world_info: &mut WorldInfo, map: &mut Map, game_obj_db: &mut GameObjectDB, trigger_loc: (i32, i32, i8),
        vault_loc: (i32, i32, i8)) {
    map.insert(trigger_loc, Tile::Trigger);
    map.insert(vault_loc, Tile::Gate(DoorState::Closed));
    let gate = SpecialSquare::make(Tile::Gate(DoorState::Closed), vault_loc, true, 0, game_obj_db);
    let gate_id = gate.obj_id();
    game_obj_db.add(gate);

    let mut trigger = SpecialSquare::make(Tile::Trigger, trigger_loc, false, 0, game_obj_db);
    let trigger_id = trigger.obj_id();
    if let GameObjects::SpecialSquare(sq) = &mut trigger {
        sq.target = Some(gate_id);
    }
    game_obj_db.add(trigger);
    game_obj_db.listeners.insert((trigger_id, EventType::SteppedOn));
}

fn simple_triggered_gate(world_info: &mut WorldInfo, map: &mut Map, floors: &mut HashSet<(i32, i32, i8)>,
        game_obj_db: &mut GameObjectDB, vault: &Vault, level: i8) {
    // Find a place for the trigger. Probably need a bail out option after X iterations in case it's some
    // weird dungeon layout where a trigger can't be placed.
    let mut delta = 2;
    loop {
        let loc = (vault.entrance.0 + delta, vault.entrance.1, level);
        let vault_entrance = (vault.entrance.0, vault.entrance.1, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_obj_db, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        let loc = (vault.entrance.0 - delta, vault.entrance.1, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_obj_db, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        let loc = (vault.entrance.0, vault.entrance.1 + delta, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_obj_db, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        let loc = (vault.entrance.0, vault.entrance.1 - delta, level);
        if floors.contains(&loc) && !loc_in_vault(vault, loc) {
            place_simple_triggered_gate(world_info, map, game_obj_db, loc, vault_entrance);
            floors.remove(&loc);
            floors.remove(&vault_entrance);
            break;
        }
        delta += 1;
    }
}

fn light_triggered_gate(_world_info: &mut WorldInfo, map: &mut Map, floors: &mut HashSet<(i32, i32, i8)>,
        game_obj_db: &mut GameObjectDB, vault: &Vault, level: i8) {
    let vault_loc = (vault.entrance.0, vault.entrance.1, level);
    map.insert(vault_loc, Tile::Gate(DoorState::Closed));
    let gate = SpecialSquare::make(Tile::Gate(DoorState::Closed), vault_loc, true, 0, game_obj_db);
    let gate_id = gate.obj_id();
    game_obj_db.add(gate);
    game_obj_db.listeners.insert((gate_id, EventType::LitUp));
    floors.remove(&vault_loc);
}

fn add_vault(world_info: &mut WorldInfo, map: &mut Map, floors: &mut HashSet<(i32, i32, i8)>,
            game_obj_db: &mut GameObjectDB, vaults: &[Vault], level: i8) {
    // In the real game, I want to make sure I never create a gated vault in a room with the upstairs 
    // because that would result in a dungeon where the player probably can't progress without magic
    let mut rng = rand::thread_rng();
    let vault_num = rng.gen_range(0, vaults.len());
    let vault = &vaults[vault_num];
    
    //simple_triggered_gate(world_info, map, floors, game_objs, vault, level);
    light_triggered_gate(world_info, map, floors, game_obj_db, vault, level);
}

fn decorate_levels(world_info: &mut WorldInfo, map: &mut Map, deepest_level: i8, floor_sqs: &mut HashMap<usize, HashSet<(i32, i32, i8)>>,
            game_obj_db: &mut GameObjectDB, vaults: HashMap<usize, Vec<Vault>>) {
    //let mut rng = rand::thread_rng();
    let mut curr_level = deepest_level;
    while curr_level > 0 {
        if curr_level < 3 {
            add_fire_pit(curr_level as usize, map, floor_sqs, game_obj_db)             
        }

        if curr_level == 1 {
            add_shrine(world_info, curr_level as usize, map, floor_sqs, game_obj_db)
        }

        if !vaults[&(curr_level as usize - 1)].is_empty() {
            let floors = floor_sqs.get_mut(&(curr_level as usize - 1)).unwrap();
            add_vault(world_info, map, floors, game_obj_db, &vaults[&(curr_level as usize - 1)], curr_level);
        }

        add_teleport_trap(curr_level as usize, floor_sqs, game_obj_db);
        
        curr_level -= 1;
    }
}

fn populate_levels(_world_info: &mut WorldInfo, deepest_level: i8, floor_sqs: &HashMap<usize, HashSet<(i32, i32, i8)>>,
            game_obj_db: &mut GameObjectDB, monster_fac: &MonsterFactory) {
    let mut curr_level = deepest_level;

    while curr_level > 0 {
        let level_index = curr_level as usize - 1;

        for _ in 0..10 {
            let loc = random_sq(&floor_sqs[&level_index]);
            monster_fac.monster_for_dungeon(loc, game_obj_db);
        }
        curr_level -= 1;
    }
}

fn find_room_id(rooms: &[HashSet<(i32, i32)>], pt: &(i32, i32)) -> Option<usize> {
    for room_id in 0..rooms.len() {
        if rooms[room_id].contains(pt) {
            return Some(room_id);
        }
    }

    None
}

// Moar floodfill -- I wonder if it's worth trying to amalgamate the 
// places I've implemented floodfill in the code.
fn connect_rooms(sqs: &mut Vec<Tile>, height: usize, width: usize) {
    let mut rooms = Vec::new();

    // find start point
    let mut q = VecDeque::new();
    let mut visited = HashSet::new();
    for j in 0..sqs.len() {
        if sqs[j] == Tile::Wall || sqs[j]  == Tile::GraniteWall{ continue; }
        let r = j / width;
        let c = j - r * width;
        let start = (r as i32, c as i32);
        if visited.contains(&start) {
            continue;
        }

        q.push_front(start);
        while !q.is_empty() {
            let pt = q.pop_front().unwrap();
            if visited.contains(&pt) {
                continue;
            }
            
            visited.insert(pt);
    
            // Find out it pt is in an existing cave, if not start a new cave
            let room_d = if let Some(id) = find_room_id(&rooms, &pt) {
                id 
            } else {
                rooms.push(HashSet::new());
                let room_id = rooms.len() - 1;            
                rooms[room_id].insert(pt);
                room_id
            };
            
            for adj in util::ADJ.iter() {
                let n = (pt.0 + adj.0, pt.1 + adj.1);
                if n.0 < 0 || n.1 < 0 || n.0 as usize >= height || n.1 as usize >= width {
                    continue;
                }
                let i = n.0 as usize * width + n.1 as usize;
                if !(sqs[i] == Tile::Wall || sqs[i] == Tile::GraniteWall) {
                    rooms[room_d].insert(n);
                    if !visited.contains(&n) {
                        q.push_back(n);
                    }
                }
            }
        }
    }

    // Just fill in any small caves that are 3 squares or less
    let mut largest = 0;
    let mut largest_id = 0;
    for j in 0..rooms.len() {
        if rooms[j].len() > largest {
            largest = rooms[j].len();
            largest_id = j;
        }
        if rooms[j].len() <= 3 {
            for sq in &rooms[j] {
                sqs[sq.0 as usize * width + sq.1 as usize] = Tile::Wall;
            }
        }
    }
    rooms.retain(|c| c.len() > 3);

    // Okay, now for each cave aside from the main one, find the sq nearest the main
    // and draw a tunnel to it.
    let mut main_area: Vec<(i32, i32)> = rooms[largest_id].iter().map(|s| *s).collect();
    main_area.sort_unstable();
    let mut room_ids = (0..rooms.len()).collect::<Vec<usize>>();
    room_ids.retain(|v| *v != largest_id);

    // Not super great in a Big-Oh sense but modern computers are fast and the grids I'm
    // dealing with are pretty small.
    if !room_ids.is_empty() {
        for id in room_ids {
            let mut closest_pt = (-1, -1);
            let mut shortest_d = i32::MAX;
            let mut start = (-1, -1);
            for pt in rooms[id].iter() {
                let (best, d) = closest_point(pt, &main_area);
                if d < shortest_d {
                    shortest_d = d;
                    closest_pt = best;
                    start = *pt;
                }
            }

            let tunnel = util::bresenham(start.0, start.1, closest_pt.0, closest_pt.1);
            // For the tunnels, I want to carve out pts that have only diagonal movement between them
            for j in 0..tunnel.len() - 1 {
                let pt = tunnel[j];
                sqs[pt.0 as usize * width + pt.1 as usize] = Tile::StoneFloor;
                let npt = tunnel[j + 1];
                if pt.0 != npt.0 && pt.1 != npt.1 {
                    let i = if pt.0 < npt.0 {
                        (pt.0 + 1) as usize * width + pt.1 as usize                        
                    } else {
                        (pt.0 - 1) as usize * width + pt.1 as usize
                    };

                    sqs[i] = Tile::StoneFloor;
                }
            }
        }
    }
}

fn closest_point(pt: &(i32, i32), room: &[(i32, i32)]) -> ((i32, i32), i32) {
    let mut shortest = i32::MAX;
    let mut nearest = *room.get(0).unwrap();
    for other_pt in room.iter() {
        let d = (pt.0 - other_pt.0) * (pt.0 - other_pt.0) + (pt.1 - other_pt.1) * (pt.1 - other_pt.1);
        if d < shortest {
            nearest = *other_pt;
            shortest = d;
        }
    }

    (nearest, shortest)
}

fn count_neighbours(sqs: &[bool], row: usize, col: usize, height: usize, width: usize) -> u8 {
    let mut sum = 0;
    for adj in util::ADJ.iter() {
        let nr = row as i32 + adj.0;
        let nc = col as i32 + adj.1;
        if nr < 0 || nc < 0 || nr as usize >= height || nc as usize >= width {
            continue;
        }

        if sqs[nr as usize * width + nc as usize] {
            sum += 1;
        }
    }

    sum
}

fn cave_overlay(width: usize, height: usize) -> Vec<bool> {
    // Classic ellular automata to make a cave system I can draw over part of a level
    let mut rng = rand::thread_rng();
    let mut sqs: Vec<bool> = (0..width * height).map(|_|  rng.gen_range(0.0, 1.0) < 0.45).collect();
    
    for _ in 0..3 {
        let mut next_gen = sqs.clone();
        for r in 0..height {
            for c in 0..width {
                let i = r * width + c;
                let count = count_neighbours(&sqs, r, c, height, width);
                if sqs[i] && count < 4 {
                    next_gen[i] = false;
                } else if !sqs[i] && count > 3 {
                    next_gen[i] = true;
                }
            }
        }

        sqs = next_gen.clone();
    }
        
    sqs
}

// My idea for the 'release' version is to have a chance of a level being
// partially filled in by caves, representing a level where there's been
// an earthquake or such that caused a cave-in. Villagers may mention a feeling
// a tremor as a hint of what's to come. Some other ideas:
//         - strew the area with rubble (once I decide how it'll effect the player)
//         - maybe graveyards or more undead to reflect that a disaster happened?
fn add_caves_to_level(tiles: &mut Vec<Tile>, height: usize, width: usize) {
    let mut rng = rand::thread_rng();
    let caves_width = rng.gen_range(40, 80);
    let caves = cave_overlay(caves_width, height - 2);
    let start_col = rng.gen_range(20, width - caves_width);

    for r in 0..height-2 {
        for c in 0..caves_width {
            let oi = r * caves_width + c;
            let map_i = (r + 1) * width + c + start_col;

            // Make some of the tiles of the cave system rubble
            tiles[map_i] = if !caves[oi] {
                Tile::Wall
            } else if rng.gen_range(0.0, 1.0) < 0.2 {
                Tile::Dirt // this will later be replaced with Rubble items
            } else {
                Tile::StoneFloor
            };
        }
    }
}

fn next_row(row: i32, distance: i32, slope: f32) -> i32 {
    let next = row as f32 + (distance as f32 * slope) / f32::sqrt(1.0 + slope * slope);

    f32::round(next) as i32
}

fn next_col(col: i32, distance: i32, slope: f32) -> i32 {
    let next = col as f32 + distance as f32 / (f32::sqrt(1.0 + slope * slope));

    f32::round(next) as i32
}

fn add_river_to_level(tiles: &mut Vec<Tile>, height: usize, width: usize, top:bool, tile: Tile) {
    // Let's say the 4 outer walls of the level are split and the river and start in any of them, so there are 6
    // different possibilities for start position and slope
    let mut rng = rand::thread_rng();
    
    let (mut row, mut col, mut slope) = if  top {
        let col = rng.gen_range(5, width / 2) as i32;
        (1, col, 1.0)
    } else {
        let col = rng.gen_range(width / 4 , width - width / 4) as i32;
        (height as i32 - 2, col, -1.0)
    };

    // So keep drawing short line segments and tweaking the slope a bit until we hit another border
    let mut river: Vec<(i32, i32)> = Vec::new();
    loop {
        let length = rng.gen_range(2, 5);
        let next_r = next_row(row, length, slope);
        let next_c = next_col(col, length, slope);
        let mut pts = util::bresenham(row, col, next_r, next_c);
        river.append(&mut pts);
        row = next_r;
        col = next_c;
        slope += rng.gen_range(-0.33, 0.33);
        if next_r < 1 || next_r > height as i32 - 2 || next_c < 1 || next_c > width as i32 - 2 {
            break;
        }
    }
    river.dedup(); // remove consecutive, repeated elements
    
    let mut pts_drawn = Vec::new();
    for pt in river.iter() {
        if pt.0 < 1 || pt.0 > height as i32 - 2 || pt.1 < 1 || pt.1 > width as i32 - 2 {
            break;
        }
        let i = pt.0 as usize * width + pt.1 as usize;
        if tiles[i] == Tile::UndergroundRiver {
            break;
        }
        tiles[i] = tile;
        pts_drawn.push(i);     
    }

    // now fatten up the river and maybe add river banks
    for pt in pts_drawn {
        tiles[pt + 1] = Tile::UndergroundRiver;
        if pt - 1 > 2 && tiles[pt - 1] != Tile::UndergroundRiver && rng.gen_range(0.0, 1.0) < 0.75 {
            tiles[pt - 1] = Tile::StoneFloor;
        }
        if pt + 2 < width - 3 && tiles[pt + 2] != Tile::UndergroundRiver && rng.gen_range(0.0, 1.0) < 0.75 {
            tiles[pt + 2] = Tile::StoneFloor;
        }
    }
}

fn build_test_dungeon(world_info: &mut WorldInfo, map: &mut Map, entrance: (i32, i32, i8), game_obj_db: &mut GameObjectDB, monster_fac: &MonsterFactory) {
    for row in entrance.0-1..entrance.0+8 {
        for col in entrance.1-1..entrance.1+8 {
            map.insert((row, col, 1), Tile::StoneFloor);
        }
    }

    for col in entrance.1-1..entrance.1+9 {
        map.insert((entrance.0-1, col, 1), Tile::Wall);
        map.insert((entrance.0+8, col, 1), Tile::Wall);
    }

    for row in entrance.0-1..entrance.0+8 {
        map.insert((row, entrance.1-1, 1), Tile::Wall);
        map.insert((row, entrance.1+4, 1), Tile::Wall);
        map.insert((row, entrance.1+8, 1), Tile::Wall);
    }

    map.insert((entrance.0+3, entrance.1+4, 1), Tile::Door(DoorState::Locked));

    //Item::mushroom(game_obj_db, (entrance.0 + 6, entrance.1 + 3, 1));
    //Item::mushroom(game_obj_db, (entrance.0 + 7, entrance.1 + 3, 1));
    Item::mushroom(game_obj_db, (entrance.0 + 7, entrance.1 + 2, 1));
    
    let loc = (entrance.0 + 3, entrance.1 + 5, 1);
    monster_fac.monster("fungal growth", loc, game_obj_db);
    map.insert((entrance.0, entrance.1, 1), Tile::StairsUp);
}

fn build_dungeon(world_info: &mut WorldInfo, map: &mut Map, entrance: (i32, i32, i8), game_obj_db: &mut GameObjectDB, monster_fac: &MonsterFactory) {
    let mut rng = rand::thread_rng();
    let width = 125;
    let height = 40;
    let mut floor_sqs = HashMap::new();
    let mut vaults = HashMap::new();
    let max_level = 5;
    let mut dungeon = Vec::new();

    let mut river_levels = Vec::new();
    for n in 0..max_level {
        let result = dungeon::draw_level(width, height);
        let mut level = result.0;
        
        // A few of the levels will have caves and/or rivers
        if n > 1 && rng.gen_range(0, 5) == 0 {
            add_caves_to_level(&mut level, height, width);
            connect_rooms(&mut level, height, width);
            world_info.facts.push(Fact::new("caves".to_string(), 0, (0, 0, n as i8 + 1)));
        }
        if n > 1 && rng.gen_range(0, 2) == 0 {
            // I should guarantee some means of crossing the river further up the dungeon
            add_river_to_level(&mut level, height, width, true, Tile::UndergroundRiver);
            if rng.gen_range(0, 3) == 0 {
                add_river_to_level(&mut level, height, width, false, Tile::UndergroundRiver);
            }
            world_info.facts.push(Fact::new("river".to_string(), 0, (0, 0, n as i8 + 1)));
            river_levels.push(n);
        }
        
        // TODO: I should probably clean out vaults that are mostly destroyed by caves
        dungeon.push(level);
        floor_sqs.insert(n, HashSet::new());
        vaults.insert(n, result.1); // vaults are rooms with only one entrance, which are useful for setting puzzles        
    }
    println!("Rivers on: {:?}", river_levels);

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

                if let Tile::Door(_) = dungeon[lvl][i] {
                    let roll = rand::thread_rng().gen_range(0.0, 1.0);
                    if roll < 0.2 {
                        map.insert((curr_row, curr_col, lvl as i8 + 1), Tile::Door(DoorState::Locked));
                    } else if roll < 0.5 {
                        map.insert((curr_row, curr_col, lvl as i8 + 1), Tile::Door(DoorState::Open));
                    } else {
                        map.insert((curr_row, curr_col, lvl as i8 + 1), dungeon[lvl][i]);
                    }
                } else if dungeon[lvl][i] == Tile::Dirt {
                    map.insert((curr_row, curr_col, lvl as i8 + 1), Tile::StoneFloor);
                    let r = Item::rubble(game_obj_db, (curr_row, curr_col, lvl as i8 + 1));
                    game_obj_db.add(r);
                } else {
                    map.insert((curr_row, curr_col, lvl as i8 + 1), dungeon[lvl][i]);
                }
                
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

    //decorate_levels(world_info, map, max_level as i8, &mut floor_sqs, game_obj_db, vaults);
    populate_levels(world_info, max_level as i8, &floor_sqs, game_obj_db, monster_fac);
    seed_items(max_level, &floor_sqs, game_obj_db);

    // if there is a river on a level, make sure the player is able to find a way to cross it on 
    // an earlier level
    for river_on in river_levels.iter() {
        let level = rand::thread_rng().gen_range(1, river_on);
        let loc = random_sq(&floor_sqs[&level]);

        let mut item = if rand::thread_rng().gen_range(0.0, 1.0) < 0.75 {
            Item::get_item(game_obj_db, "potion of levitation").unwrap()
        } else {
            Item::get_item(game_obj_db, "wand of frost").unwrap()            
        };

        item.set_loc(loc);
        game_obj_db.add(item);
    }
}

fn seed_items(deepest_level: usize, floor_sqs: &HashMap<usize, HashSet<(i32, i32, i8)>>, game_obj_db: &mut GameObjectDB) {
    for lvl in 0..deepest_level {
        for _ in 0..5 {
            let sq = random_sq(&floor_sqs[&lvl]);
            let roll = rand::thread_rng().gen_range(0.0, 1.0);
            let mut i = if roll < 0.20 {
                Item::get_item(game_obj_db, "potion of healing").unwrap()
            } else if roll < 0.4 {
                Item::get_item(game_obj_db, "torch").unwrap()
            } else if roll < 0.5 {
                Item::get_item(game_obj_db, "shield").unwrap()
            } else if roll < 0.6 {
                Item::get_item(game_obj_db, "scroll of blink").unwrap()
            } else if roll < 0.7 {
                Item::get_item(game_obj_db, "longsword").unwrap()
            } else if roll < 0.8 {
                Item::get_item(game_obj_db, "scroll of protection").unwrap()
            } else {
                let amt = rand::thread_rng().gen_range(10, 21);
                GoldPile::make(game_obj_db, amt, (0, 0, 0))
            };

            i.set_loc(sq);
            game_obj_db.add(i);
        }
    }
}

pub fn generate_world(game_obj_db: &mut GameObjectDB, monster_fac: &MonsterFactory, player_name: &str) -> (Map, WorldInfo) {
    let map_start = Instant::now();
    let mut map = wilderness::gen_wilderness_map();
    let map_end = map_start.elapsed();
    println!("Time to make world map: {:?}", map_end);

    let town_start = Instant::now();
    let mut world_info = town::create_town(&mut map, game_obj_db);
    let town_end = town_start.elapsed();
    world_info.player_name = player_name.to_string();
    println!("Town creation done {:?}", town_end);

    let valleys = find_all_valleys(&map);
    println!("Found all the valleys");
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
    println!("Found a good dungeon entrance");

    let dungeon_start = Instant::now();
    build_dungeon(&mut world_info, &mut map, dungeon_entrance, game_obj_db, monster_fac);
    //build_test_dungeon(&mut world_info, &mut map, dungeon_entrance, game_obj_db, monster_fac);
    let dungeon_end = dungeon_start.elapsed();
    println!("Time to make dungeon: {:?}", dungeon_end);

    world_info.facts.push(Fact::new("dungeon location".to_string(), 0, dungeon_entrance));

    add_old_road(&mut map, dungeon_entrance);
    map.insert((dungeon_entrance.0 as i32, dungeon_entrance.1 as i32, 0), Tile::Portal);
    
    (map, world_info)
}