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

extern crate rand;
extern crate sdl2;
extern crate serde;

use std::collections::HashMap;
use std::collections::HashSet;
use serde::{Serialize, Deserialize};
use rand::Rng;

use super::{EventResponse, EventType, FOV_HEIGHT, FOV_WIDTH, GameObjects, GameState, Map};

use crate::actor::Villager;
use crate::dialogue::DialogueLibrary;
use crate::fov;
use crate::game_obj::{GameObject, GameObjType};
use crate::items::{GoldPile, Item};
use crate::player::Player;
use crate::util;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum ShrineType {
	Woden,
	Crawler,
}

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum DoorState {
	Open,
	Closed,
	Locked,
	Broken,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Copy, Serialize, Deserialize)]
pub enum Tile {
	Blank,
	Wall,
	WoodWall,
	Door(DoorState),
	Tree,
	Dirt,
	Bridge,
	Grass,
	Player((u8, u8, u8)),
	Water,
	DeepWater,
	WorldEdge,
	Sand,
	Mountain,
	SnowPeak,
	Gate(DoorState),
	StoneFloor,
	ColourFloor((u8, u8, u8)),
	Creature((u8, u8, u8), char), // ie., NPCs
	Thing((u8, u8, u8), (u8, u8, u8), char), // ie., items
	Separator,
	Bullet(char),
	Lava,
	FirePit,
	OldFirePit(u8),
	Floor,
	Window(char),
	Spring,
    Portal,
    Fog,
	BoulderTrap((u8, u8, u8), bool, bool, (usize, usize), (i32, i32)),
	StairsUp,
	StairsDown,
	Shrine(ShrineType),
	Trigger,
}

impl Tile {
	pub fn clear(&self) -> bool {
		match self {
			Tile::Wall | Tile::Blank | Tile::Mountain | Tile::SnowPeak |
			Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) | Tile::WoodWall => false,
			_ => true,
		}
	}

	pub fn passable(&self) -> bool {
		match self {
			Tile::Wall | Tile::Blank | Tile::WorldEdge |
			Tile::Mountain | Tile::SnowPeak | Tile::Gate(DoorState::Closed) | Tile::Gate(DoorState::Locked) |
			Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) | Tile::WoodWall | Tile::Window(_) => false,		
			_ => true,
		}
	}

	pub fn passable_dry_land(&self) -> bool {
		match self {
			Tile::Wall | Tile::Blank | Tile::WorldEdge |
			Tile::Mountain | Tile::SnowPeak | Tile::Gate(DoorState::Closed) | Tile::Gate(DoorState::Locked) | 
			Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) | Tile::WoodWall | Tile::Window(_) | 
			Tile::DeepWater => false,		
			_ => true,
		}
	}

	pub fn indoors(&self) -> bool {
		match self {
			Tile::Floor | Tile::StoneFloor | Tile::StairsUp | Tile::StairsDown => true,
			_ => false,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialSquare {
	object_id: usize,
	tile: Tile,
	location: (i32, i32, i8),
	active: bool,
	radius: u8,
	pub target: Option<usize>,
}

impl SpecialSquare {
	pub fn new(object_id: usize, tile: Tile, location: (i32, i32, i8), active: bool, radius: u8) -> SpecialSquare {
		SpecialSquare { object_id, tile, location, active, radius, target: None }
	}

	fn mark_aura(&self, state: &mut GameState) {
		if self.active {
			let in_aura = fov::calc_fov(state, self.location, self.radius, FOV_HEIGHT, FOV_WIDTH, true);
			for sq in in_aura {
				if sq.1 {
					state.aura_sqs.insert(sq.0);
					state.lit_sqs.insert(sq.0);
				}
			}
		}
	}

	fn handle_triggered_event(&mut self, state: &mut GameState) {
		match self.get_tile() {
			Tile::Gate(_) => {
				self.active = !self.active;
				state.write_msg_buff("You here a metallic grinding.");
				if self.active {
					state.queued_events.push_back((EventType::GateClosed, self.location, self.object_id));
					state.map.insert(self.get_location(), Tile::Gate(DoorState::Closed));
				} else {
					state.map.insert(self.get_location(), Tile::Gate(DoorState::Open));
					state.queued_events.push_back((EventType::GateOpened, self.location, self.object_id));
				}
			},
			_ => { },
		}
	}

	// This is assuming a special square will only have this function called when it's signed up to be
	// alerted about being lit/unlit.
	fn handle_litup(&mut self, state: &mut GameState, lit: bool) {
		match self.get_tile() {
			Tile::Gate(_) => {
				// An active Gate is a closed gate
				if lit && self.active {
					self.handle_triggered_event(state);					
				} else if !lit && !self.active {
					self.handle_triggered_event(state);
				}				
			},
			_ => { },
		}
	}
}

impl GameObject for SpecialSquare {
	fn blocks(&self) -> bool {
		false
	}

    fn get_location(&self) -> (i32, i32, i8) {
		self.location
	}

    fn set_location(&mut self, _loc: (i32, i32, i8)) {
		panic!("Shouldn't be called for tiles!")
	}
	
    fn receive_event(&mut self, event: EventType, state: &mut GameState) -> Option<EventResponse> {
		match event {
			EventType::EndOfTurn => {
				self.mark_aura(state);
			},
			EventType::SteppedOn => {
				state.write_msg_buff("Click.");
				self.active = !self.active;

				if let Some(target) = self.target {
					return Some(EventResponse::new(target, EventType::Triggered));
				}
			},
			EventType::LitUp => {
				let lit = state.lit_sqs.contains(&self.location);
				self.handle_litup(state, lit);
			},
			EventType::Triggered => {
				self.handle_triggered_event(state);
			}
			_ => { 
				panic!("This event isn't implemented for special squares!");
			}
		}

		None
	}

    fn get_fullname(&self) -> String {
		panic!("Shouldn't be called for tiles!")
	}

    fn get_object_id(&self) -> usize {
		self.object_id
	}

    fn get_type(&self) -> GameObjType {
		GameObjType::SpecialSquare
	}
    fn get_tile(&self) -> Tile {
		self.tile.clone()
	}

    fn take_turn(&mut self, _state: &mut GameState, _game_objs: &mut GameObjects) {

	}

    fn is_npc(&self) -> bool {
		false
	}

    fn talk_to(&mut self, _state: &mut GameState, _player: &Player, _dialogue: &DialogueLibrary) -> String {
		panic!("Shouldn't be called for tiles!")
	}

    fn hidden(&self) -> bool {
		true
	}

    fn reveal(&mut self) {

	}

    fn hide(&mut self) {

	}

    fn as_item(&self) -> Option<Item> {
		None
	}

    fn as_zorkmids(&self) -> Option<GoldPile> {
		None
	}

    fn as_villager(&self) -> Option<Villager> {
		None
	}

	fn as_special_sq(&self) -> Option<SpecialSquare> {
        Some(self.clone())
    }
}

pub fn adjacent_door(map: &Map, loc: (i32, i32, i8), door_state: DoorState) -> Option<(i32, i32, i8)> {
	let mut doors = 0;
	let mut door: (i32, i32, i8) = (0, 0, 0);
	for r in -1..2 {
		for c in -1..2 {
			if r == 0 && c == 0 {
				continue;
			}

			let dr = loc.0 as i32 + r;
			let dc = loc.1 as i32 + c;
			let loc = (dr, dc, loc.2);
			match map[&loc] {
				Tile::Door(state) => {
					if state == door_state {
						doors += 1;
						door = loc;
					}
				},
				_ => { }
			}
		}
	}

	if doors == 1 {
		Some(door)
	} else {
		None
	}
}

// Probably at some point in the dev process, I'll need to begin 
// storing the map in a struct with extra info instead of just
// a matrix of Tiles. Then, I won't have to recalculate height and
// width every time I call the in_bounds() method
pub fn iin_bounds(map: &Vec<Vec<Tile>>, r: i32, c: i32) -> bool {
	let height = map.len() as i32;
	let width = map[0].len() as i32;

	r >= 0 && c >= 0 && r < height && c < width
}

fn find_isolated_caves(grid: &Vec<Vec<bool>>, width: usize, depth: usize) -> Vec<i32> {
	let mut ds: Vec<i32> = vec![-1; width * depth];

	// Run through the grid and union and adjacent floors
	for r in 1..depth - 1 {
		for c in 1..width - 1 {
			if grid[r][c] { continue; }
			let v = (r * width + c) as i32;
		
			if !grid[r - 1][c] { util::ds_union(&mut ds, v, v - width as i32); }
			if !grid[r + 1][c] { util::ds_union(&mut ds, v, v + width as i32); }
			if !grid[r][c - 1] { util::ds_union(&mut ds, v, v - 1); }
			if !grid[r][c + 1] { util::ds_union(&mut ds, v, v + 1); }
		}
	}

	ds
}

fn find_sets(grid: &Vec<Vec<bool>>, ds: &mut Vec<i32>, width: usize, depth: usize) -> HashMap<i32, i32> {
	let mut sets: HashMap<i32, i32> = HashMap::new();
	for r in 1..depth - 1 {
		for c in 1..width - 1 {
			if grid[r][c] { continue; }
			let v = (r * width + c) as i32;
			let root = util::ds_find(ds, v);
			let set = sets.entry(root).or_insert(0);
			*set += 1;
		}
	}

	sets
}

// The caves generated by the cellular automata method can end up disjoint --
// ie., smaller caves separated from each other. First, we need to group the
// floor squares together into sets (or equivalence classes? Is that the term?) 
// using a Disjoint Set ADT.
//
// I'm going to treat squares as adjacent only if they are adjacent along the 
// 4 cardinal compass points.
// 
// To join caves, I look for any wall squares that are separating two different
// caves, then remove them. After that, I'll fill in any smaller caves that are
// still disjoint. (In testing, this still results in decent sized maps. And 
// filling them in means when placing dungeon featuers I can assume any two floor
// squares remaining are accessible from each other.
fn cave_qa(grid: &mut Vec<Vec<bool>>, width: usize, depth: usize) {
	let mut ds = find_isolated_caves(grid, width, depth);

	// Okay, my method to join rooms is to look for single walls that
	// are separating two caves, remove them, and union the two sets.
	// After that I'll fill in any smaller leftover caves
	for r in 1..depth - 1 {
		for c in 1..width - 1 {
			if !grid[r][c] { continue; }
			let i = (r * width + c) as i32;
			let mut adj_sets = HashSet::new();	
			let mut nf = false;
			let mut sf = false;
			let mut ef = false;
			let mut wf = false;

			if !grid[r - 1][c] { 
				adj_sets.insert(util::ds_find(&mut ds, i - width as i32));
				nf = true;
			}
						
			if !grid[r + 1][c] { 
				adj_sets.insert(util::ds_find(&mut ds, i + width as i32));
				sf = true;
			}

			if !grid[r][c - 1] { 
				adj_sets.insert(util::ds_find(&mut ds, i - 1));
				wf = true;
			}

			if !grid[r][c + 1] { 
				adj_sets.insert(util::ds_find(&mut ds, i + 1));
				ef = true;
			}

			if adj_sets.len() > 1 {
				grid[r][c] = false;
				if nf { util::ds_union(&mut ds, i, i - width as i32); }
				if sf { util::ds_union(&mut ds, i, i + width as i32); }
				if wf { util::ds_union(&mut ds, i, i - 1); }
				if ef { util::ds_union(&mut ds, i, i + 1); }
			}
		}
	}

	let sets = find_sets(grid, &mut ds, width, depth);
	let mut largest_set = 0;
	let mut largest_count = 0;
	for s in sets {
		if s.1 > largest_count { 
			largest_set = s.0; 
			largest_count = s.1;
		}
	}

	for r in 1..depth - 1 {
		for c in 1..width - 1 {
			if grid[r][c] { continue; }
			let set = util::ds_find(&mut ds, (r * width + c) as i32);
			if set != largest_set {
				grid[r][c] = true;
			}
		}
	}
}

fn count_neighbouring_walls(grid: &Vec<Vec<bool>>, row: i32, col: i32, width: i32, depth: i32) -> u32 {
	let mut adj_walls = 0;

	for r in -1..2 {
		for c in -1..2 {
			let nr = row + r;
			let nc = col + c;
			if nr < 0 || nc < 0 || nr == depth || nc == width {
				adj_walls += 1;
			} else if !(nr == 0 && nc == 0) && grid[nr as usize][nc as usize] {
				adj_walls += 1;
			}
		}
	}	

	adj_walls
}

pub fn generate_test_map() -> Vec<Vec<Tile>> {
	let mut grid = vec![vec![Tile::Wall; 20]; 20];

	for r in 1..19 {
		for c in 1..19 {
			grid[r][c] = Tile::Floor;
		}
	}

	grid[1][6] = Tile::Wall;
	grid[2][6] = Tile::Wall;
	grid[3][6] = Tile::Wall;
	grid[4][6] = Tile::Wall;
	grid[5][6] = Tile::Wall;
	grid[6][6] = Tile::Wall;

	grid
}

pub fn generate_cave(width: usize, depth: usize) -> Vec<Vec<Tile>> {
	let mut grid = vec![vec![true; width]; depth];

	// Set some initial squares to be floors (false indidcates floor in our
	// initial grid)
	for r in 0..depth {
		for c in 0..width {
			let x: f64 = rand::thread_rng().gen();
			if x < 0.55 {
				grid[r][c] = false;
			}
		}
	}

	// We are using the 4-5 rule here (if a square has
	// 3 or fewer adjacents walls, it starves and becomes a floor,
	// if it has greater than 5 adj walls, it becomes a wall, otherwise
	// we leave it alone.
	//
	// One generation seems to generate nice enough maps!
	let mut next_gen = vec![vec![false; width]; depth];
	for r in 1..depth - 1 {
		for c in 1..width - 1 {
			let adj_walls = count_neighbouring_walls(&grid, r as i32, c as i32, width as i32, depth as i32);

			if adj_walls < 4 {
				next_gen[r][c] = false;
			} else if adj_walls > 5 {
				next_gen[r][c] = true;
			} else {
				next_gen[r][c] = grid[r][c];
			}
		}
	}

	// set the border
	for c in 0..width {
		next_gen[0][c] = true;
		next_gen[depth - 1][c] = true;	
	}
	for r in 1..depth - 1 {
		next_gen[r][0] = true;
		next_gen[r][width - 1] = true;
	}

	cave_qa(&mut next_gen, width, depth);

	let mut map: Vec<Vec<Tile>> = Vec::new();
	for r in next_gen {
		let mut row = Vec::new();
		for sq in r {
			let tile = if sq {
				Tile::Wall
			} else {
				Tile::StoneFloor
			};
			row.push(tile);
		}
		map.push(row);
	}
	
	map
}
