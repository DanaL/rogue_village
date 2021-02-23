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

use super::Map;

use crate::map::{DoorState, Tile};
use crate::world::WILDERNESS_SIZE;

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

fn draw_building(map: &mut Map, r: i32, c: i32, loc: (i32, i32), template: &Vec<char>) {
    let mut rng = rand::thread_rng();
    let start_r = r + 12 * loc.0;
    let start_c = c + 12 * loc.1;

    let mut building = template.clone();
    // We want to rotate the building so that an entrance more or less points toward town
    // centre (or at least doesn't face away). This code is assuming all building templates
    // have an entrance on their south wall.
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

    // Lots are 12x12 and building templates are 9x9 so we can stagger them on the lot a bit
    let stagger_r = rng.gen_range(0, 3) as i32;
    let stagger_c = rng.gen_range(0, 3) as i32;

    for row in 0..9 {
        for col in 0..9 {
            // I should add a mix of stone and wood buildings
            let tile = match building[row * 9 + col] {
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
        }
    }
}

// Town is laid out with 5x3 lots, each lot being 12x12 squares
fn place_town(map: &mut Map, buildings: &HashMap<&str, Vec<char>>) {
    let mut rng = rand::thread_rng();
    // pick starting co-ordinates that are in the centre-ish part of the map
	let start_r = rng.gen_range(WILDERNESS_SIZE /4 , WILDERNESS_SIZE / 2);
	let start_c = rng.gen_range(WILDERNESS_SIZE /4 , WILDERNESS_SIZE / 2);

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
    
    // The town will have only 1 shrine. (Maybe in the future I can implement religious rivalries...)
    let loc = available_lots.iter().choose(&mut rng).unwrap().clone();
    available_lots.remove(&loc);
    draw_building(map, start_r as i32, start_c as i32, loc, &buildings["shrine"]);

    for _ in 0..6 {
        let loc = available_lots.iter().choose(&mut rng).unwrap().clone();
        available_lots.remove(&loc);
        if rng.gen_range(0.0, 1.0) < 0.5 {
            draw_building(map, start_r as i32, start_c as i32, loc, &buildings["cottage 1"]);
        } else {
            draw_building(map, start_r as i32, start_c as i32, loc, &buildings["cottage 2"]);
        }
    }
}

pub fn create_town(map: &mut Map) {
    // load the building templates
    let mut buildings = HashMap::new();
    let contents = fs::read_to_string("buildings.txt")
        .expect("Unable to find building templates file!");
    let lines = contents.split('\n').collect::<Vec<&str>>();
    for j in 0..lines.len() / 10 {
        let name = lines[j * 10];
        let mut building = Vec::new();
        
        for r in j * 10 + 1..j * 10 + 10 {
            building.extend(lines[r].chars());            
        }
        buildings.insert(name, building);
    }

    place_town(map, &buildings);   
}