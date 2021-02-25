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
use std::fs;

use rand::Rng;
use rand::seq::IteratorRandom;

use super::{Map, NPCTable};

use crate::actor;
use crate::actor::{Actor, AgendaItem, Venue, Villager};
use crate::map::{DoorState, Tile};
use crate::pathfinding;
use crate::world::WILDERNESS_SIZE;
use crate::world::WorldInfo;

enum BuildingType {
    Shrine,
    Home,
    Tavern,
}

#[derive(Clone, Debug)]
pub struct TownBuildings {
    pub shrine: HashSet<(i32, i32, i8)>,
    pub tavern: HashSet<(i32, i32, i8)>,
    pub homes: Vec<HashSet<(i32, i32, i8)>>,
    pub taken_homes: Vec<usize>,
}

impl TownBuildings {
    pub fn new() -> TownBuildings {
        TownBuildings { shrine: HashSet::new(), tavern: HashSet::new(), homes: Vec::new(), taken_homes: Vec::new() }
    }

    pub fn vacant_home(&self) -> Option<usize> {
        if self.taken_homes.len() == self.homes.len() {
            None
        } else {
            let mut available: Vec<usize> = (0..self.homes.len()).collect();
            for x in &self.taken_homes {
                available.remove(*x);
            }
            
            let mut rng = rand::thread_rng();
            let n = available.iter().choose(&mut rng).unwrap();
            Some(*n)
        }
    }
}

fn lot_has_water(map: &Map, start_r: i32, start_c: i32, lot_r: i32, lot_c: i32) -> bool {
    for r in 0..12 {
		for c in 0..12 {
            if map[&(start_r + (lot_r * 12) + r, start_c + (lot_c * 12) + c, 0)] == Tile::DeepWater {
                return true;
            }
        }
    }

    false
}

fn rotate(building: &Vec<char>) -> Vec<char> {
    let mut rotated = building.clone();

    for r in 0..9 {
        for c in 0..9 {
            let nr = -(c as i32 - 4) + 4;
            let nc = r as i32;

            let i = (r * 9 + c) as usize;
            let ni = (nr * 9 + nc) as usize;
            if building[i] == '|' {
                rotated[ni] = '-';
            } else if building[i] == '-' {
                rotated[ni] = '|';
            } else {
                rotated[ni] = building[i];
            }
        }
    }

    rotated
}

fn draw_building(map: &mut Map, r: i32, c: i32, loc: (i32, i32), lot_width: usize, lot_length: usize,
        template: &Vec<char>, buildings: &mut TownBuildings, cat: BuildingType) {
    let mut rng = rand::thread_rng();
    let start_r = r + 12 * loc.0;
    let start_c = c + 12 * loc.1;

    let is_tavern = if let BuildingType::Tavern = cat {
        true
    } else {
        false
    };

    let mut building = template.clone();
    // We want to rotate the building so that an entrance more or less points toward town
    // centre (or at least doesn't face away). This code is assuming all building templates
    // have an entrance on their south wall.
    if !is_tavern {
        if loc.0 == 0 && loc.1 < 2 {
            if rng.gen_range(0.0, 1.0) < 0.5 {
                // make entrance face east
                building = rotate(&building);
            }
        } else if loc.0 == 0 && loc.1 > 2 {
            if rng.gen_range(0.0, 1.0) < 0.5 {
                // make entrance face west
                building = rotate(&building);
                building = rotate(&building);
                building = rotate(&building);
            }
        } else if loc.0 == 1 && loc.1 < 2 {
            // make entrance face east
            building = rotate(&building);
        } else if loc.0 == 1 && loc.1 > 2 {
            // make entrance face west
            building = rotate(&building);
            building = rotate(&building);
            building = rotate(&building);
        } else if loc.0 == 2 && loc.1 == 2 {
            // make entrance face north
            building = rotate(&building);
            building = rotate(&building);
        } else if loc.0 == 2 && loc.1 < 2 {
            if rng.gen_range(0.0, 1.0) < 0.5 {
                // make entrance face east
                building = rotate(&building);
            } else {
                // make building face north
                building = rotate(&building);
                building = rotate(&building);
            }
        } else if loc.0 == 2 && loc.1 > 2 {
            if rng.gen_range(0.0, 1.0) < 0.5 {
                // make entrance face west
                building = rotate(&building);
                building = rotate(&building);
                building = rotate(&building);
            } else {
                // make building face north
                building = rotate(&building);
                building = rotate(&building);
            }
        }
    }

    // Lots are 12x12 and building templates are 9x9 so we can stagger them on the lot a bit
    let stagger_r = rng.gen_range(0, 3) as i32;
    let stagger_c = rng.gen_range(0, 3) as i32;

    let mut building_sqs = HashSet::new();
    let is_wood = if rng.gen_range(0.0, 1.0) < 0.7 {
        true
    } else {
        false
    };
    for row in 0..lot_length {
        for col in 0..lot_width {
            let tile = match building[row * lot_width + col] {
                '#' if is_wood => Tile::WoodWall,
                '#' => Tile::Wall,
                '`' => Tile::Grass,
                '+' => Tile::Door(DoorState::Closed),
                '|' => Tile::Window('|'),
                '-' => Tile::Window('-'),
                'T' => Tile::Tree,
                '.' => Tile::StoneFloor,
                _ => panic!("Illegal character in building template!"),
            };
            let coord = (start_r + stagger_r + row as i32, start_c + stagger_c + col as i32, 0);            
            map.insert(coord, tile);

            match map[&coord] {
                Tile::Door(_) => { building_sqs.insert(coord); },
                Tile::Floor => { building_sqs.insert(coord); },
                Tile::StoneFloor => { building_sqs.insert(coord); },
                _ => {  false; },
            }
        }
    }

    match cat {
        BuildingType::Shrine => buildings.shrine = building_sqs,
        BuildingType::Home => buildings.homes.push(building_sqs),
        BuildingType::Tavern => buildings.tavern = building_sqs,
    }
}

fn place_tavern(map: &mut Map, r: i32, c: i32, templates: &HashMap<String, Vec<char>>, buildings: &mut TownBuildings) -> ((i32, i32), (i32, i32)) {
    let mut rng = rand::thread_rng();
    let x = rng.gen_range(0, 4);

    if x == 0 {
        // east facting tavern
        let lot_r = rng.gen_range(0, 2);
        let lot_c = rng.gen_range(0, 2);
        draw_building(map, r, c, (lot_r, lot_c),9,18, &templates["tavern 1"], buildings, BuildingType::Tavern);
        ((lot_r, lot_c), (lot_r + 1, lot_c))
    } else if x == 1 {
        // south facing tavern
        let lot_r = 0;
        let lot_c = rng.gen_range(0, 4);
        draw_building(map, r, c, (lot_r, lot_c),18,9, &templates["tavern 2"], buildings, BuildingType::Tavern);
        ((lot_r, lot_c), (lot_r, lot_c + 1))
    } else if x == 2 {
        // north facing tavern
        let lot_r = 2;
        let lot_c = rng.gen_range(0, 4);
        draw_building(map, r, c, (lot_r, lot_c),18,9, &templates["tavern 3"], buildings, BuildingType::Tavern);
        ((lot_r, lot_c), (lot_r, lot_c + 1))
    } else {
        // west facing tavern
        let lot_r = rng.gen_range(0, 2);
        let lot_c = rng.gen_range(3, 5);
        draw_building(map, r, c, (lot_r, lot_c),18,9, &templates["tavern 4"], buildings, BuildingType::Tavern);
        ((lot_r, lot_c), (lot_r + 1, lot_c))
    }
}

// Town is laid out with 5x3 lots, each lot being 12x12 squares
fn place_town_buildings(map: &mut Map, start_r: usize, start_c: usize, 
            templates: &HashMap<String, Vec<char>>, buildings: &mut TownBuildings) {   
    let mut rng = rand::thread_rng();

    // Step one, get rid of most but not all of the trees in town and replace with grass.
	for r in start_r..start_r + 36 {
		for c in start_c..start_c + 60 {
            if map[&(r as i32, c as i32, 0)] == Tile::Tree && rng.gen_range(0.0, 1.0) < 0.85 {
                map.insert((r as i32, c as i32, 0), Tile::Grass);
            }
        }
    }

    let mut available_lots = HashSet::new();
	for r in 0..3 {
		for c in 0..5 {
			// Avoid lots with water in the them to avoid plunking a house
			// over a river. This is pretty simple minded and  I could do something 
			// fancier like actually checking if placing a house will overlap with water 
			// so that if there is just a corner or edge that's water it's still good. Maybe 
			// in Real CodeTM. Also should reject a town placement where there aren't enough 
			// lots for all the buildings I want to add because of water hazards.
			if r == 1 && c == 2 { continue; } // leave the centre sq empty as a 'town square'
			if !lot_has_water(map, start_r as i32, start_c as i32, r, c) {
				available_lots.insert((r, c));
            }
        }
    }
    
    // Pick and and place one of the inn templates, which take up two lots
    let lots_used = place_tavern(map, start_r as i32, start_c as i32, templates, buildings);
    available_lots.remove(&lots_used.0);
    available_lots.remove(&lots_used.1);

    // The town will have only 1 shrine. (Maybe in the future I can implement relisgious rivalries...)
    let loc = available_lots.iter().choose(&mut rng).unwrap().clone();
    available_lots.remove(&loc);
    draw_building(map, start_r as i32, start_c as i32, loc,9,9, &templates["shrine"], buildings, BuildingType::Shrine);

    for _ in 0..6 {
        let loc = available_lots.iter().choose(&mut rng).unwrap().clone();
        available_lots.remove(&loc);
        if rng.gen_range(0.0, 1.0) < 0.5 {
            draw_building(map, start_r as i32, start_c as i32, loc, 9, 9, &templates["cottage 1"], buildings, BuildingType::Home);
        } else {
            draw_building(map, start_r as i32, start_c as i32, loc, 9, 9, &templates["cottage 2"], buildings, BuildingType::Home);
        }
    }
}

// Draw paths in town. For now they just converge on the town square but I might in the future have
// some of them move from one neighbour to another
fn draw_paths_in_town(map: &mut Map, world_info: &WorldInfo) {
    let mut rng = rand::thread_rng();
    let mut doors = HashSet::new();

    let adj: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];
    for r in world_info.town_boundary.0..world_info.town_boundary.2 {
        for c in world_info.town_boundary.1..world_info.town_boundary.3 {
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
    passable.insert(Tile::Tree, 2.0);
    passable.insert(Tile::Water, 3.0);
    passable.insert(Tile::DeepWater, 3.0);
    let j = rng.gen_range(0, world_info.town_square.len());
    let centre = world_info.town_square.iter().nth(j).unwrap();
    for door in doors {
        let path = pathfinding::find_path(map, false, door.0, door.1, 0, centre.0, centre.1, 150, &passable);
        if path.len() > 0 {
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

// eventually to be fancier
fn random_tavern_name() -> String {
    let mut rng = rand::thread_rng();
    let nouns = ["Arms", "Boar", "Cup", "Axe", "Bow", "Elf", "Stag"];
    let adjective = ["Black", "Golden", "Broken", "Jeweled", "Lost"];

    let noun = String::from(*nouns.iter().choose(&mut rng).unwrap());
    let adj = String::from(*adjective.iter().choose(&mut rng).unwrap());

    let tavern = format!("the {} {}", adj, noun);

    tavern
}

fn random_town_name() -> String {
    let mut rng = rand::thread_rng();

    // This will one day be more fancy and expansive...
    let names = ["Skara Brae", "Jhelom", "Yew", "Moonglow", "Magincia", "Antioch"];

    String::from(*names.iter().choose(&mut rng).unwrap())
}

fn create_villager(voice: &str, tb: &mut TownBuildings, used_names: &HashSet<String>) -> Villager {
    let home_id = tb.vacant_home().unwrap();
    let home = &tb.homes[home_id];
    let j = rand::thread_rng().gen_range(0, home.len());    
    let loc = home.iter().nth(j).unwrap().clone(); 
    let mut villager = Villager::new(actor::pick_villager_name(used_names), loc, home_id, voice);
    tb.taken_homes.push(home_id);
    
    if voice.starts_with("mayor") {
        villager.schedule.push(AgendaItem::new((9, 0), (21, 0), 0, Venue::TownSquare));
        villager.schedule.push(AgendaItem::new((12, 0), (13, 0), 10, Venue::Tavern));
    } else {
        villager.schedule.push(AgendaItem::new((11, 0), (14, 0), 10, Venue::Tavern));
        villager.schedule.push(AgendaItem::new((18, 0), (22, 0), 10, Venue::Tavern));
    }

    villager
}

pub fn create_town(map: &mut Map, npcs: &mut NPCTable) -> WorldInfo {
    // load the building templates
    let mut buildings = HashMap::new();
    let contents = fs::read_to_string("buildings.txt")
        .expect("Unable to find building templates file!");
    let lines = contents.split('\n').collect::<Vec<&str>>();
    let mut curr_building = String::from("");
    for line in lines {
        if line.starts_with('%') {
            curr_building = line[1..].to_string();
            buildings.insert(curr_building.to_string(), Vec::new());
        } else {
            buildings.get_mut(&curr_building).unwrap().extend(line.chars());
        }
    }

    let mut rng = rand::thread_rng();
    // pick starting co-ordinates that are in the centre-ish part of the map
	let start_r = rng.gen_range(WILDERNESS_SIZE /4 , WILDERNESS_SIZE / 2);
	let start_c = rng.gen_range(WILDERNESS_SIZE /4 , WILDERNESS_SIZE / 2);

    let mut tb = TownBuildings::new();
    place_town_buildings(map, start_r, start_c, &buildings, &mut tb);

    let tavern_name = random_tavern_name();
    let town_name = random_town_name();
    let mut world_info = WorldInfo::new(town_name.to_string(),
        (start_r as i32, start_c as i32, start_r as i32 + 35, start_c as i32 + 60),
        tavern_name.to_string());    
    
    // The town square is in lot (1, 2)
    for r in start_r + 12..start_r + 24 {
        for c in start_c + 24..start_c + 36 {
            if map[&(r as i32, c as i32, 0)].passable_dry_land() {
                world_info.town_square.insert((r as i32, c as i32, 0));
            }
        }
    }

    draw_paths_in_town(map, &world_info);

    let mut used_names = HashSet::new();
    let v = create_villager("mayor1", &mut tb, &used_names);
    used_names.insert(v.get_name());
    npcs.insert(v.stats.location, Box::new(v));
    
    let v = create_villager("villager1", &mut tb, &used_names);
    used_names.insert(v.get_name());
    npcs.insert(v.stats.location, Box::new(v));

    world_info.town_buildings = Some(tb);

    world_info
}