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
use std::fs;

use rand::{Rng, prelude::SliceRandom};
use rand::seq::IteratorRandom;
use serde::{Serialize, Deserialize};

use super::{EventType, Map};

use crate::actor;
use crate::actor::{AgendaItem, Venue, NPC};
use crate::game_obj::{GameObject, GameObjects};
use crate::map::{DoorState, Tile};
use crate::pathfinding;
use crate::world::WILDERNESS_SIZE;
use crate::world::WorldInfo;

const TOWN_WIDTH: i32 = 60;
const TOWN_HEIGHT: i32 = 36;

#[derive(Debug)]
enum BuildingType {
    Shrine,
    Home,
    Tavern,
}

#[derive(Debug)]
pub struct Template {
    pub sqs: Vec<char>,
    pub width: usize,
    pub height: usize,
}

impl Template {
    pub fn new(width: usize, height: usize) -> Template {
        Template { width, height, sqs: Vec::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

fn count_water_sqs(map: &Map, start_r: i32, start_c: i32, lot_r: i32, lot_c: i32) -> u8 {
    let mut sum = 0;
    for r in 0..12 {
		for c in 0..12 {            
            if map[&(start_r + (lot_r * 12) + r, start_c + (lot_c * 12) + c, 0)] == Tile::DeepWater {
                sum += 1;
            }
        }
    }

    sum
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

fn draw_building(map: &mut Map, loc: (i32, i32), town: (i32, i32), template: &Template,
        buildings: &mut TownBuildings, cat: &BuildingType) {
    let mut rng = rand::thread_rng();    
    let mut building_sqs = HashSet::new();
    let is_wood = rng.gen_range(0.0, 1.0) < 0.7;

    let building = match cat {
        BuildingType::Tavern => template.sqs.clone(),
        _ => {
            // if the building isn't the tavern, we might want to rotate it
            let centre_row = loc.0 + template.height as i32 / 2;
            let centre_col = loc.1 + template.width as i32 / 2;
            let mut sqs = template.sqs.clone();
            
            let quarter = TOWN_HEIGHT / 4;
            let north_quarter = town.0 + quarter;
            let south_quarter = town.0 + quarter + quarter;
            let mid = (town.1 + TOWN_WIDTH as i32) / 2;

            if centre_row >= south_quarter { 
                // rotate doors to face north
                sqs = rotate(&sqs);
                sqs = rotate(&sqs);
            } else if centre_row > north_quarter && centre_col < mid {
                // rotate doors to face east
                sqs = rotate(&sqs);
            } else if centre_row > north_quarter && centre_col > mid {
                // rotate doors to face west
                sqs = rotate(&sqs);
                sqs = rotate(&sqs);
                sqs = rotate(&sqs);
            }

            sqs
        },
    };
    
    for r in 0..template.height {
        for c in 0..template.width {
            let coord = (loc.0 + r as i32, loc.1 + c as i32, 0);
            let tile = match building[r as usize * template.width + c as usize] {
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
            map.insert(coord, tile);

            match map[&coord] {
                Tile::Door(_) => { building_sqs.insert(coord); },
                Tile::Floor => { building_sqs.insert(coord); },
                Tile::StoneFloor => { building_sqs.insert(coord); },
                _ => { },
            }
        }
    }

    match cat {
        BuildingType::Shrine => buildings.shrine = building_sqs,
        BuildingType::Home => buildings.homes.push(building_sqs),
        BuildingType::Tavern => buildings.tavern = building_sqs,
    }
    /*
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

    
    
    }

    */
}

fn building_fits(map: &mut Map, nw_r: i32, nw_c: i32, template: &Template) -> bool {
    for r in 0..template.height {
        for c in 0..template.width {
            let loc = (nw_r + r as i32, nw_c + c as i32, 0);
            match &map[&loc] {
                Tile::DeepWater | Tile::Wall | Tile::WoodWall | Tile::Window(_) |
                Tile::Floor | Tile::StoneFloor | Tile::Door(_) => { return false; }
                _ => { continue; }
            }
        }
    }
    
    // We also want to ensure there is a little space between buildings
    for c in 0..template.width {
        let loc = (nw_r - 1, nw_c + c as i32, 0);
        if map[&loc] == Tile::Wall || map[&loc] == Tile::WoodWall {
            return false;
        } 
        let loc = (nw_r + template.height as i32, nw_c + c as i32, 0);
        if map[&loc] == Tile::Wall || map[&loc] == Tile::WoodWall {
            return false;
        } 
    }

    for r in 0..template.height {
        let loc = (nw_r + r as i32, nw_c - 1, 0);
        if map[&loc] == Tile::Wall || map[&loc] == Tile::WoodWall {
            return false;
        }
        let loc = (nw_r + r as i32, nw_c + template.width as i32, 0);
        if map[&loc] == Tile::Wall || map[&loc] == Tile::WoodWall {
            return false;
        }
    }

    true
}

fn check_along_col(map: &mut Map, start: (i32, i32), delta: i32, town: (i32, i32), template: &Template, buildings: &mut TownBuildings, cat: &BuildingType) -> bool {
    let height = template.height as i32;

    if delta > 0 {
        let mut row = start.0;
        while row + height < town.0 + TOWN_HEIGHT {
            if building_fits(map, row, start.1, template) {
                draw_building(map, (row, start.1), town, template, buildings, cat);
                return true;
            }

            row += delta;
        }
    } else {
        let mut row = start.0 - height;
        while row > town.0 {
            if building_fits(map, row, start.1, template) {
                draw_building(map, (row, start.1), town, template, buildings, cat);
                return true;
            }
        }

        row += delta;
    }

    false
}

fn check_along_row(map: &mut Map, start: (i32, i32), delta: i32, town: (i32, i32), template: &Template, buildings: &mut TownBuildings, cat: &BuildingType) -> bool {
    let width = template.width as i32;

    if delta > 0 {
        let mut col = start.1;
        while col + width < town.1 + TOWN_WIDTH {
            if building_fits(map, start.0, col, template) {
                draw_building(map, (start.0, col), town, template, buildings, cat);
                return true;
            }
            col += delta;
        }
    } else {
        let mut col = town.1 + TOWN_WIDTH - width - 1;
        while col > town.1 {
            if building_fits(map, start.0, col, template) {
                draw_building(map, (start.0, col), town, template, buildings, cat);
                return true;
            }
            col += delta;
        }
    }

    false
}

// The inn is placed on the outside of town
fn place_tavern(map: &mut Map, town_r: i32, town_c: i32, templates: &HashMap<String, Template>, buildings: &mut TownBuildings) {
    let mut rng = rand::thread_rng();
    let mut options = vec![1, 2, 3, 4];
    options.shuffle(&mut rng);

    while !options.is_empty() {
        let choice = options.pop().unwrap();

        if choice == 1 {
            // east facting tavern
            let template = templates.get("tavern 1").unwrap();
            let (start_r, delta) = if rng.gen_range(0.0, 1.0) < 0.5 {
                (town_r, 1)
            } else {
                (town_r + TOWN_HEIGHT, -1)
            };
            if check_along_col(map, (start_r, town_c), delta, (town_r, town_c), template, buildings, &BuildingType::Tavern) {
                break;
            }
        } else if choice == 2 {
            // south facing tavern
            let template = templates.get("tavern 2").unwrap();
            let (start_c, delta) = if rng.gen_range(0.0, 1.0) < 0.5 {
                (town_c, 1)
            } else {
                (town_c + TOWN_WIDTH - template.width as i32, - 1)
            };
            if check_along_row(map, (town_r, start_c), delta, (town_r, town_c), template, buildings, &BuildingType::Tavern) {
                break;
            }
        } else if choice == 3 {
            // north facing tavern
            let template = templates.get("tavern 3").unwrap();
            let (start_c, delta) = if rng.gen_range(0.0, 1.0) < 0.5 {
                (town_c, 1)
            } else {
                (town_c + TOWN_WIDTH - template.width as i32, - 1)
            };
            if check_along_row(map, (town_r + TOWN_HEIGHT - template.height as i32 - 1, start_c), delta, (town_r, town_c), template, buildings, &BuildingType::Tavern) {
                break;
            }
        } else {
            // west facing tavern
            let template = templates.get("tavern 4").unwrap();
            let (start_r, delta) = if rng.gen_range(0.0, 1.0) < 0.5 {
                (town_r, 1)
            } else {
                (town_r + TOWN_HEIGHT, -1)
            };
            if check_along_col(map, (start_r, town_c + TOWN_WIDTH - template.width as i32 - 1), delta, (town_r, town_c), template, buildings, &BuildingType::Tavern) {
                break;
            }
        }
    }
}

fn place_building(map: &mut Map, town_r: i32, town_c: i32, template: &Template, buildings: &mut TownBuildings, cat: BuildingType) -> bool {
    let mut rng = rand::thread_rng();
    let mut options = vec![1, 2, 3, 4];
    options.shuffle(&mut rng);
    
    while !options.is_empty() {
        let pick = options.pop().unwrap();

        if pick == 1 {
            // Start at the top left
            let (mut row, mut col, delta_r, delta_c) = (town_r, town_c, 2, 2);
            loop {
                if check_along_row(map, (row, col), delta_c, (town_r, town_c), template, buildings, &cat) {
                    return true;
                }
                row += delta_r;
                col += delta_c;
                if col + template.width as i32 > town_c + TOWN_WIDTH {
                    col = town_c;
                }

                if row < town_r || row + template.height as i32 > town_r + TOWN_HEIGHT {
                    break;
                }
            }
        } else if pick == 2 {
            // Start at the bottom left
            let (mut row, mut col, delta_r, delta_c) = (town_r + TOWN_HEIGHT as i32 - template.height as i32 - 1, 
                    town_c, -2, 2);
            loop {
                if check_along_row(map, (row, col), delta_c, (town_r, town_c), template, buildings, &cat) {
                    return true;
                }
                row += delta_r;
                col += delta_c;
                if col + template.width as i32 > town_c + TOWN_WIDTH {
                    col = town_c;
                }

                if row < town_r || row + template.height as i32 > town_r + TOWN_HEIGHT {
                    break;
                }
            }
        } else if pick == 3 {
            // Start at the top right
            let (mut row, mut col, delta_r, delta_c) = (town_r, town_c + TOWN_WIDTH - template.width as i32 - 1, 2, -2);
            loop {
                if check_along_row(map, (row, col), delta_c, (town_r, town_c), template, buildings, &cat) {
                    return true;
                }
                row += delta_r;
                col += delta_c;
                if col < town_c {
                    col = town_c + TOWN_WIDTH - template.width as i32 - 1;
                }

                if row < town_r || row + template.height as i32 > town_r + TOWN_HEIGHT {
                    break;
                }
            } 
        } else {
            // Start at bottom right
            let (mut row, mut col, delta_r, delta_c) = (town_r + TOWN_HEIGHT - template.height as i32 - 1, 
                town_c + TOWN_WIDTH - template.width as i32 - 1, -2, -2);
            loop {
                if check_along_row(map, (row, col), delta_c, (town_r, town_c), template, buildings, &cat) {
                    return true;
                }
                row += delta_r;
                col += delta_c;
                if col < town_c {
                    col = town_c + TOWN_WIDTH - template.width as i32 - 1;
                }

                if row < town_r || row + template.height as i32 > town_r + TOWN_HEIGHT {
                    break;
                }
            }
        }
    }

    false
}

// Town is laid out with 5x3 lots, each lot being 12x12 squares
fn place_town_buildings(map: &mut Map, town_r: i32, town_c: i32, 
            templates: &HashMap<String, Template>, buildings: &mut TownBuildings) {   
    let mut rng = rand::thread_rng();

    // Step one, get rid of most but not all of the trees in town and replace with grass.
	for r in town_r..town_r + TOWN_HEIGHT {
		for c in town_c..town_c + TOWN_WIDTH {
            // if map[&(r, c, 0)] == Tile::Tree && rng.gen_range(0.0, 1.0) < 0.85 {
            //     map.insert((r, c, 0), Tile::Grass);
            // }
            map.insert((r, c, 0), Tile::Dirt);
        }
    }

    // Start by placing the tavern since it's the largest building and the hardest to fit
    place_tavern(map, town_r, town_c, templates, buildings);
    
    // The town will have only 1 shrine. (Maybe in the future I can implement religious rivalries...)
    place_building(map, town_r, town_c, &templates["shrine"], buildings, BuildingType::Shrine);

    for _ in 0..8 {
        let template = if rng.gen_range(0.0, 1.0) < 0.5 {
            &templates["cottage 1"]
        } else {
            &templates["cottage 2"]
        };
        if !place_building(map, town_r, town_c, template, buildings, BuildingType::Home) {
            break;
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
        let path = pathfinding::find_path(map, None, false, door.0, door.1, 0, centre.0, centre.1, 150, &passable);
        if !path.is_empty() {
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

fn create_villager(voice: &str, tb: &mut TownBuildings, used_names: &HashSet<String>, game_objs: &mut GameObjects) -> GameObject {
    let home_id = tb.vacant_home().unwrap();
    let home = &tb.homes[home_id];
    let j = rand::thread_rng().gen_range(0, home.len());    
    let loc = home.iter().nth(j).unwrap();
    let mut villager = NPC::villager(actor::pick_villager_name(used_names), *loc, home_id, voice, game_objs);
    tb.taken_homes.push(home_id);
    
    if voice.starts_with("mayor") {
        villager.npc.as_mut().unwrap().schedule.push(AgendaItem::new((9, 0), (21, 0), 0, Venue::TownSquare));
        villager.npc.as_mut().unwrap().schedule.push(AgendaItem::new((12, 0), (13, 0), 10, Venue::Tavern));
    } else {
        villager.npc.as_mut().unwrap().schedule.push(AgendaItem::new((11, 0), (14, 0), 10, Venue::Tavern));
        villager.npc.as_mut().unwrap().schedule.push(AgendaItem::new((18, 0), (22, 0), 10, Venue::Tavern));
    }

    villager
}

pub fn create_town(map: &mut Map, game_objs: &mut GameObjects) -> WorldInfo {
    // load the building templates
    let mut buildings = HashMap::new();
    let contents = fs::read_to_string("buildings.txt")
        .expect("Unable to find building templates file!");
    let lines = contents.split('\n').collect::<Vec<&str>>();
    let mut curr_building: String = "".to_string();
    let mut width: usize = 0;
    let mut sqs: Vec<char> = Vec::new();
    let mut rows: usize = 0;
    for line in lines {
        if line.starts_with('%') {            
            if !curr_building.is_empty() {
                let mut template = Template::new(width, rows);
                template.sqs = sqs.clone();
                buildings.insert(curr_building.to_string(), template);
            }

            curr_building = line[1..].to_string();
            rows = 0;
            sqs = Vec::new();
        } else {
            width = line.len();
            sqs.extend(line.chars());
            rows += 1;
        }
    }
    let mut template = Template::new(width, rows);
    template.sqs = sqs.clone();
    buildings.insert(curr_building.to_string(), template);

    let mut rng = rand::thread_rng();
    // // pick starting co-ordinates that are in the centre-ish part of the map
	let start_r = rng.gen_range(WILDERNESS_SIZE /4 , WILDERNESS_SIZE / 2);
	let start_c = rng.gen_range(WILDERNESS_SIZE /4 , WILDERNESS_SIZE / 2);

    let mut tb = TownBuildings::new();
    place_town_buildings(map, start_r as i32, start_c as i32, &buildings, &mut tb);

    println!("{:?}", tb.tavern);
    println!("# of homes: {}", tb.homes.len());
    
    let tavern_name = random_tavern_name();
    let town_name = random_town_name();
    let mut world_info = WorldInfo::new(town_name,
        (start_r as i32, start_c as i32, start_r as i32 + 35, start_c as i32 + 60),
        tavern_name);    
    
    // The town square is in lot (1, 2)
    // for r in start_r + 12..start_r + 24 {
    //     for c in start_c + 24..start_c + 36 {
    //         if map[&(r as i32, c as i32, 0)].passable_dry_land() {
    //             world_info.town_square.insert((r as i32, c as i32, 0));
    //         }
    //     }
    // }

    // draw_paths_in_town(map, &world_info);

    // let mut used_names = HashSet::new();
    // let v = create_villager("mayor1", &mut tb, &used_names, game_objs);
    // used_names.insert(v.get_fullname());
    // let obj_id = v.object_id;    
    // game_objs.add(v);
    // game_objs.listeners.insert((obj_id, EventType::TakeTurn));

    // let v = create_villager("villager1", &mut tb, &used_names, game_objs);
    // used_names.insert(v.get_fullname());
    // let obj_id = v.object_id;
    // game_objs.add(v);
    // game_objs.listeners.insert((obj_id, EventType::TakeTurn));
    
    // world_info.town_buildings = Some(tb);

    world_info
}