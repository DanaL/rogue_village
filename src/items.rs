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

use super::{EventResponse, EventType, GameState, Message, PLAYER_INV};

use crate::battle::DamageType;
use crate::display;
use crate::effects;
use crate::fov;
use crate::game_obj::{GameObject, GameObjectBase, GameObjectDB, GameObjects};
use crate::map::Tile;
use std::u128;
use rand::Rng;

// Some bitmasks so that I can store various extra item attributes beyond
// just the item type enum. (Ie., heavy armour, two-handed, etc)
pub const IA_LIGHT_ARMOUR: u128 = 0x00000001;
pub const IA_MED_ARMOUR: u128   = 0x00000002;
pub const IA_HEAVY_ARMOUR: u128 = 0x00000004;
pub const IA_CONSUMABLE: u128   = 0x00000010;
pub const IA_TWO_HANDED: u128   = 0x00000020;
pub const IA_IMMOBILE: u128     = 0x00000040;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum ItemType {
	Weapon,
	Food,
	Armour,
    Light,
    Zorkmid,
    Note,
    Bottle,
    Potion,
    Shield,
    Scroll,
    Obstacle,
    Ammunition,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
    pub base_info: GameObjectBase,
    pub item_type: ItemType,
	pub weight: u8,
	pub stackable: bool,
	pub slot: char,
	pub dmg_die: u8,
	pub dmg_dice: u8,
	pub attack_bonus: i8,
    pub ac_bonus: i8,
	pub range: u8,
	pub equiped: bool,
    pub attributes: u128,
    pub active: bool,
    pub charges: u16,
    pub aura: u8,
    pub text: Option<(String, String)>,
    pub dmg_type: DamageType,
    pub value: u16,
    pub effects: u128,
    pub item_dc: u8,
}

impl Item {    
    fn new(object_id: usize, symbol: char, lit_colour: (u8, u8, u8), unlit_colour: (u8, u8, u8), name: &str, item_type: ItemType, weight: u8, stackable: bool, value: u16) -> Item {
		Item { base_info: GameObjectBase::new(object_id, (-1, -1, -1), false, symbol, lit_colour, unlit_colour, false, name),
             item_type, weight, stackable, slot: '\0', dmg_die: 1, dmg_dice: 1, attack_bonus: 0, ac_bonus: 0, range: 0, equiped: false, 
                attributes: 0, active: false, charges: 0, aura: 0, text: None, dmg_type: DamageType::Bludgeoning, value, effects: 0, item_dc: 10 }								
	}
    
    pub fn get_item(game_obj_db: &mut GameObjectDB, name: &str) -> Option<GameObjects> {
        match name {
            "longsword" => {
                let mut i = Item::new(game_obj_db.next_id(), ')',display::WHITE, display::GREY, name, ItemType::Weapon, 3, false, 15);
                i.dmg_die = 8;
                i.dmg_type = DamageType::Slashing;
                
                Some(GameObjects::Item(i))
            },
            "shortsword" => {
                let mut i = Item::new(game_obj_db.next_id(), ')',display::WHITE, display::GREY, name, ItemType::Weapon, 3, false, 15);
                i.dmg_die = 6;
                i.dmg_type = DamageType::Slashing;
                
                Some(GameObjects::Item(i))
            },
            "dagger" => {
                let mut i = Item::new(game_obj_db.next_id(), ')',display::WHITE, display::GREY, name, ItemType::Weapon, 1, true, 2);
                i.dmg_die = 4;
                i.dmg_type = DamageType::Slashing;
                
                Some(GameObjects::Item(i))
            },
            "spear" => {
                let mut i = Item::new(game_obj_db.next_id(), ')',display::WHITE, display::GREY, name, ItemType::Weapon, 2, false, 2);
                i.dmg_die = 6;
                i.dmg_type = DamageType::Piercing;
                
                Some(GameObjects::Item(i))
            },
            "two-handed sword" => {
                let mut i = Item::new(game_obj_db.next_id(), ')',display::WHITE, display::GREY, name, ItemType::Weapon, 6, false, 30);
                i.dmg_die = 12;
                i.dmg_type = DamageType::Slashing;
                i.attributes |= IA_TWO_HANDED;
                
                Some(GameObjects::Item(i))
            },
            "staff" => {
                let mut i = Item::new(game_obj_db.next_id(), ')',display::LIGHT_BROWN, display::BROWN, name, ItemType::Weapon, 2, false, 2);
                i.dmg_die = 6;
                i.dmg_type = DamageType::Bludgeoning;
                
                Some(GameObjects::Item(i))
            },
            "ringmail" => {
                let mut i = Item::new(game_obj_db.next_id(), '[',display::GREY, display::DARK_GREY, name, ItemType::Armour, 10, false, 30);
                i.ac_bonus = 3;
                i.attributes |= IA_MED_ARMOUR;
                
                Some(GameObjects::Item(i))
            },
            "leather armour" => {
                let mut i = Item::new(game_obj_db.next_id(), '[',display::BROWN, display::DARK_BROWN, name, ItemType::Armour, 5, false, 5);
                i.ac_bonus = 1;
                i.attributes |= IA_LIGHT_ARMOUR;
                
                Some(GameObjects::Item(i))
            },
            "chainmail" => {
                let mut i = Item::new(game_obj_db.next_id(), '[',display::GREY, display::DARK_GREY, name, ItemType::Armour, 15, false, 75);
                i.ac_bonus = 5;
                i.attributes |= IA_MED_ARMOUR;
                
                Some(GameObjects::Item(i))
            },
            "shield" => {
                let mut i = Item::new(game_obj_db.next_id(), '[',display::GREY, display::DARK_GREY, name, ItemType::Shield, 5, false, 10);
                i.ac_bonus = 1;
                
                Some(GameObjects::Item(i))
            },         
            "torch" => {
                let mut i = Item::new(game_obj_db.next_id(), '(',display::LIGHT_BROWN, display::BROWN, name, ItemType::Light, 1, true, 1);
                i.charges = 1000;
                i.aura = 5;
                
                Some(GameObjects::Item(i))
            },
            "wineskin" => {
                let mut w = Item::new(game_obj_db.next_id(), '(',display::LIGHT_BROWN, display::BROWN, name, ItemType::Bottle, 1, false, 2);
                w.charges = 0;

                Some(GameObjects::Item(w))
            },
            "note" => {
                let i = Item::new(game_obj_db.next_id(), '?',display::WHITE, display::LIGHT_GREY, name, ItemType::Note, 0, false, 0);
                
                Some(GameObjects::Item(i))
            },
            "potion of healing" => {
                let mut i = Item::new(game_obj_db.next_id(), '!',display::WHITE, display::LIGHT_GREY, name, ItemType::Potion, 2, true, 10);
                i.attributes |= IA_CONSUMABLE;
                i.effects |= effects::EF_MINOR_HEAL;
                
                Some(GameObjects::Item(i))
            },
            "scroll of blink" => {
                let mut i = Item::new(game_obj_db.next_id(), '?',display::WHITE, display::LIGHT_GREY, name, ItemType::Scroll, 1, true, 20);
                i.attributes |= IA_CONSUMABLE;
                i.effects |= effects::EF_BLINK;
                
                Some(GameObjects::Item(i))
            }
            "arrow" => {
                let mut a = Item::new(game_obj_db.next_id(), '|', display::BROWN, display::DARK_BROWN, name, ItemType::Ammunition, 0, true, 1);
                a.dmg_dice = 1;
                a.dmg_die = 4;
                a.dmg_type = DamageType::Piercing;
                
                Some(GameObjects::Item(a))
            },
            "piece of mushroom" => {
                let mut m = Item::new(game_obj_db.next_id(), '%', display::LIGHT_BLUE, display::BLUE, name, ItemType::Food, 0, true, 0);
                m.attributes |= IA_CONSUMABLE;

                Some(GameObjects::Item(m))
            },
            _ => None,
        }
    }

    pub fn web(game_obj_db: &mut GameObjectDB, strength: u8) -> GameObjects {
        let mut web = Item::new(game_obj_db.next_id(), ':',display::WHITE, display::GREY, "web", ItemType::Obstacle, 0, false, 0);
        web.attributes |= IA_IMMOBILE;
        web.item_dc = strength;

        GameObjects::Item(web)
    }

    pub fn rubble(game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8)) -> GameObjects {
        let mut rubble = Item::new(game_obj_db.next_id(), ':',display::GREY, display::DARK_GREY, "rubble", ItemType::Obstacle, 0, false, 0);
        rubble.attributes |= IA_IMMOBILE;
        rubble.item_dc = 15;
        rubble.set_loc(loc);
        
        GameObjects::Item(rubble)
    }

    pub fn mushroom(game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8)) -> GameObjects {
        let roll = rand::thread_rng().gen_range(0.0, 1.0);
        let (lit_colour, colour) = if roll < 0.33 {
            (display::GREEN, display::DARK_GREEN)
        } else if roll < 0.66 {
            (display::PINK, display::PURPLE)        
        } else {
            (display::LIGHT_BLUE, display::BLUE)
        };

        let mut mushroom = Item::new(game_obj_db.next_id(), '"',lit_colour, colour, "mushroom", ItemType::Obstacle, 0, false, 0);
        mushroom.attributes |= IA_IMMOBILE;
        mushroom.set_loc(loc);

        GameObjects::Item(mushroom)
    }

    pub fn desc(&self) -> String {
        // Will I ever want an item that's both equiped AND active??
        if self.equiped {
            return match self.item_type {
                ItemType::Weapon =>  String::from("(in hand)"),
                ItemType::Armour => String::from("(being worn)"),
                ItemType::Shield => String::from("(on your arm)"),
                _ => "".to_string(),
            }        
        } else if self.active {
            return "(lit)".to_string()
        } else if self.item_type == ItemType::Bottle {
            return if self.charges == 1 {
                String::from("(half full")
            } else if self.charges == 2 {
                String::from("(full)")
            } else {
                String::from("(empty)")
            };      
        }

		"".to_string()
    }

    pub fn equip(&mut self) {
        self.equiped = true;
    }

    pub fn unequip(&mut self) {
        self.equiped = true;
    }

    pub fn equipable(&self) -> bool {
        matches!(self.item_type, ItemType::Armour | ItemType::Weapon | ItemType::Shield)
    }

    pub fn useable(&self) -> bool {
        self.item_type == ItemType::Light || self.item_type == ItemType::Potion ||
            self.item_type == ItemType::Scroll || self.item_type == ItemType::Food
    }

    pub fn stackable(&self) -> bool {
        if self.item_type == ItemType::Light && self.equiped {
            false
        } else {
            self.stackable
        }
    }

	fn mark_lit_sqs(&self, state: &mut GameState, loc: (i32, i32, i8), player_loc: (i32, i32, i8)) {
		let location = if loc == PLAYER_INV {
			player_loc
		} else {
			loc
		};

		let lit = fov::calc_fov(state, location, self.aura, true);
		for sq in lit {			
			state.lit_sqs.insert(sq);			
		}		
	}
}

impl GameObject for Item {
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
        let s = format!("{} {}", self.base_info.name, self.desc());
        s.trim().to_string()
    }

    fn obj_id(&self) -> usize {
        self.base_info.object_id
    }

    fn get_tile(&self) -> Tile {
        Tile::Thing(self.base_info.lit_colour, self.base_info.unlit_colour, self.base_info.symbol)
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

    fn receive_event(&mut self, event: EventType, state: &mut GameState, player_loc: (i32, i32, i8)) -> Option<EventResponse> {
        let obj_id = self.obj_id();
        let loc = self.get_loc();

        match event {
            EventType::Update => {
                // right now light sources are the only things in the game which times like this
				// This'll mark squares that are lit independent of the player's vision. Don't bother
				// with the calculation if the light source is on another level of the dungeon
                // Note this currently isn't working for a monster carrying a lit light source (or, eventually
                // other items that may have charges)
                if self.charges > 0 && (loc == PLAYER_INV || self.get_loc().2 == player_loc.2) {
                    self.mark_lit_sqs(state, loc, player_loc);
				}                
            },
			EventType::EndOfTurn => {
				self.charges -= 1;
                
				if self.charges == 150 {
					let s = if loc == PLAYER_INV {
						format!("Your {} flickers.", self.base_info.name)					
					} else {
						format!("The {} flickers.", self.base_info.name)
					};
					self.aura -= 2;
                    state.msg_queue.push_back(Message::new(obj_id, loc, &s, ""));
				} else if self.charges == 25 {
					let s = if self.get_loc() == PLAYER_INV {
						format!("Your {} seems about to go out.", self.base_info.name)					
					} else {
						format!("The {} seems about to out.", self.base_info.name)
					};
                    state.msg_queue.push_back(Message::new(obj_id, loc, &s, ""));
				} else if self.charges == 0 {
					let s = if self.get_loc() == PLAYER_INV {
						format!("Your {} has gone out!", self.base_info.name)					
					} else {
						format!("The {} has gone out!", self.base_info.name)
					};
                    state.msg_queue.push_back(Message::new(obj_id, loc, &s, ""));

                    let er = EventResponse::new(self.obj_id(), EventType::LightExpired);
					return Some(er);
				}
			},
			_ => {
				// We don't care about any other events here atm and probably should be an error
				// condition if we receive one
			},
		}

        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GoldPile {
    base_info: GameObjectBase,
    pub amount: u32,    
}

impl GoldPile {
    pub fn make(game_obj_db: &mut GameObjectDB, amount: u32, loc: (i32, i32, i8)) -> GameObjects {
        let g = GoldPile { base_info: GameObjectBase::new(game_obj_db.next_id(), loc, false, '$', display::GOLD,
            display::YELLOW_ORANGE, false, "zorkmids"), amount };
            
        GameObjects::GoldPile(g)
    }
}

impl GameObject for GoldPile {
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
        if self.amount == 1 {
            String::from("1 gold piece")
        } else {
            let s = format!("{} gold pieces", self.amount);
            s
        }
    }

    fn obj_id(&self) -> usize {
        self.base_info.object_id
    }

    fn get_tile(&self) -> Tile {
        Tile::Thing(self.base_info.lit_colour, self.base_info.unlit_colour, self.base_info.symbol)
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

    fn receive_event(&mut self, _event: EventType, _state: &mut GameState, _player_loc: (i32, i32, i8)) -> Option<EventResponse> {
        None
    }
}
