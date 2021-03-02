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

use serde::{Serialize, Deserialize};

use display::YELLOW_ORANGE;

use super::{EventType, FOV_HEIGHT, FOV_WIDTH, GameState, GameObjects, PLAYER_INV};

use crate::actor::Villager;
use crate::dialogue::DialogueLibrary;
use crate::display;
use crate::fov;
use crate::game_obj::{GameObject, GameObjType};
use crate::map::Tile;
use crate::player::Player;
use crate::util::StringUtils;

// Some bitmasks so that I can store various extra item attributes beyond
// just the item type enum. (Ie., heavy armour, two-handed, etc)
pub const IA_LIGHT_ARMOUR: u32 = 0b00000001;
pub const IA_MED_ARMOUR:   u32 = 0b00000010;
pub const IA_HEAVY_ARMOUR: u32 = 0b00000100;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum ItemType {
	Weapon,
	Zorkmid,
	Food,
	Armour,
    Light,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub object_id: usize,
    pub name: String,
	pub item_type: ItemType,
	pub weight: u8,
	pub symbol: char,
	pub lit_colour: (u8, u8, u8),
    pub unlit_colour: (u8, u8, u8),
	pub stackable: bool,
	pub slot: char,
	pub dmg_die: u8,
	pub dmg_dice: u8,
	pub attack_bonus: i8,
    pub ac_bonus: i8,
	pub range: u8,
	pub equiped: bool,
    pub attributes: u32,
    pub active: bool,
    pub charges: u16,
    pub aura: u8,
    pub location: (i32, i32, i8),
    pub text: Option<(String, String)>,
}

impl Item {    
    fn new(object_id: usize, name: &str, item_type: ItemType, weight: u8, stackable: bool, symbol: char, lit_colour: (u8, u8, u8), unlit_colour: (u8, u8, u8)) -> Item {
		Item { object_id, name: String::from(name), item_type, weight, stackable, symbol, lit_colour, unlit_colour, slot: '\0',
				dmg_die: 1, dmg_dice: 1, attack_bonus: 0, ac_bonus: 0, range: 0, equiped: false, attributes: 0, active: false, charges: 0, aura: 0,
                location: (-1, -1, -1), text: None }								
	}
    
    pub fn get_item(game_objs: &mut GameObjects, name: &str) -> Option<Item> {
        match name {
            "longsword" => {
                let mut i = Item::new(game_objs.next_id(), name, ItemType::Weapon, 3, false, ')', display::WHITE, display::GREY);
                i.dmg_die = 8;
                Some(i)
            },
            "dagger" => {
                let mut i = Item::new(game_objs.next_id(), name, ItemType::Weapon, 1, false, ')', display::WHITE, display::GREY);
                i.dmg_die = 4;
                Some(i)
            },
            "spear" => {
                let mut i = Item::new(game_objs.next_id(), name, ItemType::Weapon, 1, false, ')', display::WHITE, display::GREY);
                i.dmg_die = 6;
                Some(i)
            },
            "staff" => {
                let mut i = Item::new(game_objs.next_id(), name, ItemType::Weapon, 1, false, ')', display::LIGHT_BROWN, display::BROWN);
                i.dmg_die = 6;
                Some(i)
            },
            "ringmail" => {
                let mut i = Item::new(game_objs.next_id(), name, ItemType::Armour, 8, false, '[', display::GREY, display::DARK_GREY);
                i.ac_bonus = 3;
                i.attributes |= IA_MED_ARMOUR;                
                Some(i)
            },
            "gold piece" => {
                let i = Item::new(std::usize::MAX, name, ItemType::Zorkmid, 0, true, '$', display::GOLD, display::YELLOW_ORANGE);
                Some(i)
            },
            "torch" => {
                let mut i = Item::new(game_objs.next_id(), name, ItemType::Light, 1, true, '(', display::LIGHT_BROWN, display::BROWN);
                i.charges = 1000;
                i.aura = 5;
                Some(i)
            },
            "note" => {
                let i = Item::new(game_objs.next_id(), name, ItemType::Note, 0, false, '?', display::WHITE, display::LIGHT_GREY);
                Some(i)
            }
            _ => None,
        }
    }

    pub fn equipable(&self) -> bool {
        match self.item_type {
            ItemType::Weapon => true,
            ItemType::Armour => true,
            _ => false,
        }
    }

    pub fn useable(&self) -> bool {
        if let ItemType::Light = self.item_type {
            true
        } else {
            false
        }
    }

    pub fn stackable(&self) -> bool {
        if self.item_type == ItemType::Light && self.equiped {
            false
        } else {
            self.stackable
        }
    }

	fn mark_lit_sqs(&self, state: &mut GameState) {
		let loc = if self.location == PLAYER_INV {
			state.player_loc
		} else {
			self.location
		};

		let lit = fov::calc_fov(state, loc, self.aura, FOV_HEIGHT, FOV_WIDTH);
		for sq in lit {
			if sq.1 {
				state.lit_sqs.insert(sq.0);
			}
		}		
	}
}

impl PartialEq for Item {
	fn eq(&self, other: &Self) -> bool {
        // Comparing by charges will keep, say, torches with differing amounts
        // of turns left from stacking
		self.name == other.name && self.charges == other.charges && self.active == other.active
	}
}

impl GameObject for Item {
    fn blocks(&self) -> bool {
        false
    }

    fn is_npc(&self) -> bool {
        false
    }

    fn get_location(&self) -> (i32, i32, i8) {
        self.location
    }

    fn set_location(&mut self, loc: (i32, i32, i8)) {
        self.location = loc;
    }

    fn receive_event(&mut self, event: EventType, state: &mut GameState) -> Option<EventType> {
		match event {
			EventType::EndOfTurn => {
				self.charges -= 1;

				// right now light sources are the only things in the game which times like this
				// This'll mark squares that are lit independent of the player's vision. Don't bother
				// with the calculation if the light source is on another level of the dungeon
				if self.charges > 0 && (self.location == PLAYER_INV || self.location.2 == state.player_loc.2) {
					self.mark_lit_sqs(state);
				}

				if self.charges == 150 {
					let s = if self.location == PLAYER_INV {
						format!("Your {} flickers.", self.name)					
					} else {
						format!("The {} flickers.", self.name)
					};
					self.aura -= 2;
					state.write_msg_buff(&s);
				} else if self.charges == 25 {
					let s = if self.location == PLAYER_INV {
						format!("Your {} seems about to go out.", self.name)					
					} else {
						format!("The {} seems about to out.", self.name)
					};
					state.write_msg_buff(&s);
				} else if self.charges == 0 {
					let s = if self.location == PLAYER_INV {
						format!("Your {} has gone out!", self.name)					
					} else {
						format!("The {} hss gone out!", self.name)
					};
					state.write_msg_buff(&s);
					return Some(EventType::LightExpired);
				}
			},
			_ => {
				// We don't care about any other events here atm and probably should be an error
				// condition if we receive one
			},
		}

        None
    }

    fn get_fullname(&self) -> String {
        let mut s = String::from(&self.name);
		
        match self.item_type {
            ItemType::Weapon => if self.equiped { s.push_str(" (in hand)"); },
            ItemType::Armour => if self.equiped { s.push_str(" (being worn)"); },
            ItemType::Light =>  if self.active { s.push_str( " (lit)"); },
            _ => { },
        }

		s
    }

    fn get_object_id(&self) -> usize {
        self.object_id
    }

    fn get_tile(&self) -> Tile {
        Tile::Thing(self.lit_colour, self.unlit_colour, self.symbol)        
    }

    fn get_type(&self) -> GameObjType {
        GameObjType::Item
    }

    fn as_zorkmids(&self) -> Option<GoldPile> {
        None
    }

    fn as_item(&self) -> Option<Item> {
        Some(self.clone())
    }

    fn as_villager(&self) -> Option<Villager> {
        None
    }

    fn take_turn(&mut self, _state: &mut GameState, _game_objs: &mut GameObjects) {
         
    }

    fn talk_to(&mut self, _state: &mut GameState, _player: &Player, _dialogue: &DialogueLibrary) -> String {
        format!("You are trying to talk to {}...", self.get_fullname().with_indef_article())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldPile {
    pub object_id: usize,
    pub amount: u32,
    pub location: (i32, i32, i8),
}

impl GoldPile {
    pub fn new( object_id: usize, amount: u32, location: (i32, i32, i8)) -> GoldPile {
        GoldPile { object_id, amount, location }
    }
}

impl GameObject for GoldPile {
    fn blocks(&self) -> bool {
        false
    }

    fn is_npc(&self) -> bool {
        false
    }

    fn get_location(&self) -> (i32, i32, i8) {
        self.location
    }

    fn set_location(&mut self, loc: (i32, i32, i8)) {
        self.location = loc;
    }

    fn receive_event(&mut self, _event: EventType, _state: &mut GameState) -> Option<EventType> {
        None
    }

    fn get_fullname(&self) -> String {
        let name  = if self.amount == 1 {
            String::from("1 gold piece")
        } else {
            let s = format!("{} gold pieces", self.amount);
            s
        };

        name
    }

    fn get_object_id(&self) -> usize {
        self.object_id
    }

    fn get_tile(&self) -> Tile {
        Tile::Thing(display::GOLD,  YELLOW_ORANGE, '$')
    }

    fn get_type(&self) -> GameObjType {
        GameObjType::Zorkmids
    }

    fn as_item(&self) -> Option<Item> {
        None
    }

    fn as_zorkmids(&self) -> Option<GoldPile> {
        Some(self.clone())
    }

    fn as_villager(&self) -> Option<Villager> {
        None
    }

    fn take_turn(&mut self, _state: &mut GameState, _game_objs: &mut GameObjects) {
         
    }

    fn talk_to(&mut self, _state: &mut GameState, _player: &Player, _dialogue: &DialogueLibrary) -> String {
        String::from("You are trying to talk to a pile of money...")
    }
}
