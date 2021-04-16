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

use super::{EventResponse, EventType, GameState, Map, Message};

use crate::display;
use crate::display::Colour;
use crate::fov;
use crate::game_obj::{GameObject, GameObjectBase, GameObjectDB, GameObjects};

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
	ColourFloor(Colour),
	Creature(Colour, char), // ie., NPCs
	Thing(Colour, Colour, char), // ie., items
	Separator,
	Bullet(char),
	Lava,
	FirePit,
	Forge,
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
	UndergroundRiver,
	Well,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct SpecialSquare {
	base_info: GameObjectBase,
	tile: Tile,
	active: bool,
	radius: u8,
	pub target: Option<usize>,
}

impl SpecialSquare {
	pub fn make(tile: Tile, location: (i32, i32, i8), active: bool, radius: u8, game_obj_db: &mut GameObjectDB) -> GameObjects {
		let sq = SpecialSquare { base_info: GameObjectBase::new(game_obj_db.next_id(), location, true, ' ', display::BLACK,
			display::BLACK, false, "special sq"), tile,  radius, target: None, active };

		GameObjects::SpecialSquare(sq)
	}

	pub fn teleport_trap(location: (i32, i32, i8), game_obj_db: &mut GameObjectDB) -> GameObjects {
		let sq = SpecialSquare { base_info: GameObjectBase::new(game_obj_db.next_id(), location, true, '^', display::PINK,
			display::PURPLE, false, "teleport trap"), tile: Tile::TeleportTrap, radius: 0, target: None, active: true };

		GameObjects::SpecialSquare(sq)		
	}

	fn mark_aura(&self, state: &mut GameState, loc: (i32, i32, i8)) {
		if self.active {
			let in_aura = fov::calc_fov(state, loc, self.radius, true);
			for sq in in_aura {				
				state.aura_sqs.insert(sq);
				state.lit_sqs.insert(sq, display::LIGHT_BLUE);				
			}
		}
	}

	fn handle_triggered_event(&mut self, state: &mut GameState, loc: (i32, i32, i8), obj_id: usize) {
		if let Tile::Gate(_) = self.tile {
			self.active = !self.active;
			state.msg_queue.push_back(Message::new(obj_id, loc, "You hear a metallic grinding.", "You hear a metallic grinding."));
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
			return Some(EventResponse::new(obj_id, EventType::TrapRevealed));
		}  else {
			state.msg_queue.push_back(Message::new(obj_id, self.get_loc(), "Click.", "Click."));
			self.active = !self.active;

			if let Some(target) = self.target {
				return Some(EventResponse::new(target, EventType::Triggered));
			}
		}

		None
	}
}

impl GameObject for SpecialSquare {
	fn blocks(&self) -> bool {
		false
	}

	fn get_loc(&self) -> (i32, i32, i8) {
		self.base_info.location
	}

	fn set_loc(&mut self, loc: (i32, i32, i8)) {
		self.base_info.location = loc;
	}

	fn get_fullname(&self) -> String {
		self.base_info.name.clone()
	}

	fn obj_id(&self) -> usize {
		self.base_info.object_id
	}

	fn get_tile(&self) -> Tile {
		self.tile
	}

	fn hidden(&self) -> bool {
		self.base_info.hidden
	}

	fn hide(&mut self) {
		self.base_info.hidden = true;
	}
	fn reveal(&mut self) {
		self.base_info.hidden = false;
	}

	fn receive_event(&mut self, event: EventType, state: &mut GameState, _player_loc: (i32, i32, i8)) -> Option<EventResponse> {
		let loc = self.get_loc();
		let obj_id = self.obj_id();
		match event {
			EventType::Update => {
				self.mark_aura(state, loc);
			},
			EventType::SteppedOn => return self.stepped_on(state, obj_id),
			EventType::LitUp => {
				let lit = state.lit_sqs.contains_key(&loc);
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
