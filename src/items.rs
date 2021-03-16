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

use super::{EventResponse, EventType, GameState, GameObjects, PLAYER_INV};

use crate::battle::DamageType;
use crate::display;
use crate::fov;
use crate::game_obj::{GameObject};

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
    Bottle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
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
    pub attributes: u32,
    pub active: bool,
    pub charges: u16,
    pub aura: u8,
    pub text: Option<(String, String)>,
    pub dmg_type: DamageType,
    pub value: u16,
}

impl Item {    
    fn new(item_type: ItemType, weight: u8, stackable: bool, value: u16) -> Item {
		Item { item_type, weight, stackable, slot: '\0', dmg_die: 1, dmg_dice: 1, attack_bonus: 0, ac_bonus: 0, range: 0, equiped: false, 
                attributes: 0, active: false, charges: 0, aura: 0, text: None, dmg_type: DamageType::Bludgeoning, value, }								
	}
    
    pub fn get_item(game_objs: &mut GameObjects, name: &str) -> Option<GameObject> {
        match name {
            "longsword" => {
                let mut i = Item::new(ItemType::Weapon, 3, false, 15);
                i.dmg_die = 8;
                i.dmg_type = DamageType::Slashing;
                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), ')', display::WHITE, display::GREY, None, Some(i) , None, None, None, false);

                Some(obj)
            },
            "dagger" => {
                let mut i = Item::new(ItemType::Weapon, 1, false, 2);
                i.dmg_die = 4;
                i.dmg_type = DamageType::Slashing;
                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), ')', display::WHITE, display::GREY, None, Some(i) , None, None, None, false);

                Some(obj)
            },
            "spear" => {
                let mut i = Item::new(ItemType::Weapon, 2, false, 2);
                i.dmg_die = 6;
                i.dmg_type = DamageType::Piercing;
                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), ')', display::WHITE, display::GREY, None, Some(i) , None, None, None, false);

                Some(obj)
            },
            "staff" => {
                let mut i = Item::new(ItemType::Weapon, 1, false, 2);
                i.dmg_die = 6;
                i.dmg_type = DamageType::Bludgeoning;
                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), ')', display::LIGHT_BROWN, display::BROWN, None, Some(i) , None, None, None, false);

                Some(obj)
            },
            "ringmail" => {
                let mut i = Item::new(ItemType::Armour, 10, false, 30);
                i.ac_bonus = 3;
                i.attributes |= IA_MED_ARMOUR;
                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), '[', display::GREY, display::DARK_GREY, None, Some(i) , None, None, None, false);
                
                Some(obj)
            },            
            "torch" => {
                let mut i = Item::new(ItemType::Light, 1, true, 1);
                i.charges = 1000;
                i.aura = 5;
                
                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), '(', display::LIGHT_BROWN, display::BROWN, None, Some(i) , None, None, None, false);
                Some(obj)
            },
            "wineskin" => {
                let mut w = Item::new(ItemType::Bottle, 1, false, 1);
                w.charges = 0;

                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), '(', display::LIGHT_BROWN, display::BROWN, None, Some(w) , None, None, None, false);
                Some(obj)
            },
            "note" => {
                let i = Item::new(ItemType::Note, 0, false, 0);
                let obj = GameObject::new(game_objs.next_id(), name, (0, 0, 0), '?', display::WHITE, display::LIGHT_GREY, None, Some(i) , None, None, None, false);            
                
                Some(obj)
            }
            _ => None,
        }
    }

    pub fn desc(&self) -> String {
        // Will I ever want an item that's both equiped AND active??
        if self.equiped {
            return match self.item_type {
                ItemType::Weapon =>  String::from("(in hand)"),
                ItemType::Armour => String::from("(being worn)"),
                ItemType::Bottle => {
                    if self.charges == 1 {
                        String::from("(half full")
                    } else if self.charges == 2 {
                        String::from("(full)")
                    } else {
                        String::from("empty")
                    }
                }
                _ => "".to_string(),
            }        
        } else if self.active {
            return "(lit)".to_string()
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
        matches!(self.item_type, ItemType::Armour | ItemType::Weapon)
    }

    pub fn useable(&self) -> bool {
        self.item_type == ItemType::Light
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

    pub fn receive_event(&mut self, event: EventType, state: &mut GameState, 
                loc: (i32, i32, i8), player_loc: (i32, i32, i8),
                name: String, obj_id: usize) -> Option<EventResponse> {
		match event {
            EventType::Update => {
                // right now light sources are the only things in the game which times like this
				// This'll mark squares that are lit independent of the player's vision. Don't bother
				// with the calculation if the light source is on another level of the dungeon
                // Note this currently isn't working for a monster carrying a lit light source (or, eventually
                // other items that may have charges)
                if self.charges > 0 && (loc == PLAYER_INV || loc.2 == player_loc.2) {
                    self.mark_lit_sqs(state, loc, player_loc);
				}                
            },
			EventType::EndOfTurn => {
				self.charges -= 1;
                
				if self.charges == 150 {
					let s = if loc == PLAYER_INV {
						format!("Your {} flickers.", name)					
					} else {
						format!("The {} flickers.", name)
					};
					self.aura -= 2;
					state.write_msg_buff(&s);
				} else if self.charges == 25 {
					let s = if loc == PLAYER_INV {
						format!("Your {} seems about to go out.", name)					
					} else {
						format!("The {} seems about to out.", name)
					};
					state.write_msg_buff(&s);
				} else if self.charges == 0 {
					let s = if loc == PLAYER_INV {
						format!("Your {} has gone out!", name)					
					} else {
						format!("The {} has gone out!", name)
					};
					state.write_msg_buff(&s);

                    let er = EventResponse::new(obj_id, EventType::LightExpired);
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldPile {
    pub amount: u32,    
}

impl GoldPile {
    pub fn make(game_objs: &mut GameObjects, amount: u32, loc: (i32, i32, i8)) -> GameObject {
        let g = GoldPile { amount };
        GameObject::new(game_objs.next_id(), "zorkmids", loc, '$', display::GOLD, display::YELLOW_ORANGE, None, None , Some(g), None, None, false)
    }

    pub fn get_fullname(&self) -> String {
        if self.amount == 1 {
            String::from("1 gold piece")
        } else {
            let s = format!("{} gold pieces", self.amount);
            s
        }
    }
}
