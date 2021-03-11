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

use serde::{Serialize, Deserialize};

use super::{EventResponse, EventType, GameObjects, GameState, Map};

use crate::display;
use crate::fov;
use crate::game_obj::GameObject;

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
	TeleportTrap,
	Rubble,
	UndergroundRiver,
}

impl Tile {
	pub fn clear(&self) -> bool {
		!matches!(self,
			Tile::Wall | Tile::Blank | Tile::Mountain | Tile::SnowPeak |
			Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) | Tile::WoodWall)
	}

	pub fn passable(&self) -> bool {
		!matches!(self,
			Tile::Wall | Tile::Blank | Tile::WorldEdge |
			Tile::Mountain | Tile::SnowPeak | Tile::Gate(DoorState::Closed) | Tile::Gate(DoorState::Locked) |
			Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) | Tile::WoodWall | Tile::Window(_) |
			Tile::UndergroundRiver)
	}

	pub fn passable_dry_land(&self) -> bool {
		!matches!(self,
			Tile::Wall | Tile::Blank | Tile::WorldEdge |
			Tile::Mountain | Tile::SnowPeak | Tile::Gate(DoorState::Closed) | Tile::Gate(DoorState::Locked) | 
			Tile::Door(DoorState::Closed) | Tile::Door(DoorState::Locked) | Tile::WoodWall | Tile::Window(_) | 
			Tile::DeepWater | Tile::UndergroundRiver)
	}

	pub fn indoors(&self) -> bool {
		matches!(self, Tile::Floor | Tile::StoneFloor | Tile::StairsUp | Tile::StairsDown)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialSquare {
	tile: Tile,
	active: bool,
	radius: u8,
	pub target: Option<usize>,
}

impl SpecialSquare {
	pub fn make(tile: Tile, location: (i32, i32, i8), active: bool, radius: u8, game_objs: &mut GameObjects) -> GameObject {
		let sq = SpecialSquare { tile,  radius, target: None, active };

		let mut obj = GameObject::new(game_objs.next_id(), "special sq", location, ' ', display::BLACK, display::BLACK, None, None , None, Some(sq), None, false);
		obj.hidden = true;

		obj
	}

	pub fn teleport_trap(location: (i32, i32, i8), game_objs: &mut GameObjects) -> GameObject {
		let sq = SpecialSquare { tile: Tile::TeleportTrap,  radius: 0, target: None, active: true };

		let mut obj = GameObject::new(game_objs.next_id(), "teleport trap", location, '^', display::PINK, display::PURPLE, None, None , None, Some(sq), None, false);
		obj.hidden = true;

		obj
	}

	fn mark_aura(&self, state: &mut GameState, loc: (i32, i32, i8)) {
		if self.active {
			let in_aura = fov::calc_fov(state, loc, self.radius, true);
			for sq in in_aura {				
				state.aura_sqs.insert(sq);
				state.lit_sqs.insert(sq);				
			}
		}
	}

	fn handle_triggered_event(&mut self, state: &mut GameState, loc: (i32, i32, i8), obj_id: usize) {
		if let Tile::Gate(_) = self.tile {
			self.active = !self.active;
			state.write_msg_buff("You hear a metallic grinding.");
			if self.active {
				state.queued_events.push_back((EventType::GateClosed, loc, obj_id, None));
				state.map.insert(loc, Tile::Gate(DoorState::Closed));
			} else {
				state.map.insert(loc, Tile::Gate(DoorState::Open));
				state.queued_events.push_back((EventType::GateOpened, loc, obj_id, None));
			}
		}
	}

	// This is assuming a special square will only have this function called when it's signed up to be
	// alerted about being lit/unlit.
	fn handle_litup(&mut self, state: &mut GameState, lit: bool, loc: (i32, i32, i8), obj_id: usize) {
		if let Tile::Gate(_) = self.tile {
			if (lit && self.active) || (!lit && !self.active)  {
				self.handle_triggered_event(state, loc, obj_id);
			}
		}
	}

	fn stepped_on(&mut self, state: &mut GameState, obj_id: usize) -> Option<EventResponse> {
		if self.tile == Tile::TeleportTrap {
			state.write_msg_buff("A feeling of vertigo!");
			return Some(EventResponse::new(obj_id, EventType::TrapRevealed));
		}  else {
			state.write_msg_buff("Click.");
			self.active = !self.active;

			if let Some(target) = self.target {
				return Some(EventResponse::new(target, EventType::Triggered));
			}
		}

		None
	}

	pub fn receive_event(&mut self, event: EventType, state: &mut GameState, loc: (i32, i32, i8), obj_id: usize) -> Option<EventResponse> {
		match event {
			EventType::Update => {
				self.mark_aura(state, loc);
			},
			EventType::SteppedOn => return self.stepped_on(state, obj_id),
			EventType::LitUp => {
				let lit = state.lit_sqs.contains(&loc);
				self.handle_litup(state, lit, loc, obj_id);
			},
			EventType::Triggered => {
				self.handle_triggered_event(state, loc, obj_id);
			}
			_ => { 
				panic!("This event isn't implemented for special squares!");
			}
		}

		None
	}

	pub fn get_tile(&self) -> Tile {
		self.tile
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
			if let Tile::Door(state) = map[&loc] {
				if state == door_state {
					doors += 1;
					door = loc;
				}
			}
		}
	}

	if doors == 1 {
		Some(door)
	} else {
		None
	}
}
