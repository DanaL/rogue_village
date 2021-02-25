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

use std::collections::HashMap;

use crate::display;

#[derive(Debug, Clone)]
pub struct Inventory {
	next_slot: char,
	inv: HashMap<char, (Item, u8)>,
    pub purse: u32, // I hope I don't actually need that large an integer for money in this game...
}

impl Inventory {
    pub fn new() -> Inventory {
		Inventory { next_slot: 'a', inv: HashMap::new(), purse: 0 }
	}

    // This doesn't currently handle when all the inventory slots are used up...
    pub fn add(&mut self, item: Item) {
		if item.stackable {
			// since the item is stackable, let's see if there's a stack we can add it to
			// Super cool normal programming language way to loop over the keys of a hashtable :?
			let slots = self.inv.keys()
								.map(|v| v.clone())
								.collect::<Vec<char>>();
			for slot in slots {
				let mut val = self.inv.get_mut(&slot).unwrap();
				if val.0 == item && val.0.stackable {
					val.1 += 1;
					return;
				}
			}
		} 

		// If the last slot the item occupied is still available, use that
		// instead of the next available slot.
		if item.prev_slot != '\0' && !self.inv.contains_key(&item.prev_slot) {
			self.inv.insert(item.prev_slot, (item, 1));
		} else {
			self.inv.insert(self.next_slot, (item, 1));
			self.set_next_slot();
		}
	}

    pub fn get_menu(&self) -> Vec<String> {
		let mut menu = Vec::new();

		let mut slots = self.inv
			.keys()
			.map(|v| v.clone())
			.collect::<Vec<char>>();
		slots.sort();

        if self.purse == 1 {
            menu.push(String::from("$) a single zorkmid to your name"));
        } else if self.purse > 1 {
            menu.push(format!("$) {} gold pieces", self.purse));
        }

		for slot in slots {
			let mut s = String::from("");
			s.push(slot);
			s.push_str(") ");
			let val = self.inv.get(&slot).unwrap();
			if val.1 == 1 {
				s.push_str("a ");
				s.push_str(&val.0.get_full_name());
			} else {
				s.push_str(&val.0.get_full_name());
				s.push_str(" x");
				s.push_str(&val.1.to_string());
			}
			menu.push(s);
		}

        menu
	}

    fn set_next_slot(&mut self) {
		let mut slot = self.next_slot;
		
		loop {
			slot = (slot as u8 + 1) as char;
			if slot > 'z' {
				slot = 'a';
			}

			if !self.inv.contains_key(&slot) {
				self.next_slot = slot;
				break;
			}

			if slot == self.next_slot {
				// No free spaces left in the invetory!
				self.next_slot = '\0';
				break;
			}
		}
	}
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ItemType {
	Weapon,
	Zorkmid,
	Food,
	Armour,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
	pub item_type: ItemType,
	pub weight: u8,
	pub symbol: char,
	pub colour: (u8, u8, u8),
    pub lit_colour: (u8, u8, u8),
	pub stackable: bool,
	pub prev_slot: char,
	pub dmg_die: u8,
	pub dmg_dice: u8,
	pub bonus: u8,
	pub range: u8,
	pub armour_value: i8,
	pub equiped: bool,
}

impl Item {    
    fn new(name: &str, item_type: ItemType, weight: u8, stackable: bool, symbol: char, colour: (u8, u8, u8), lit_colour: (u8, u8, u8)) -> Item {
		Item { name: String::from(name), item_type, weight, stackable, symbol, colour, lit_colour, prev_slot: '\0',
				dmg_die: 1, dmg_dice: 1, bonus: 0, range: 0, armour_value: 0, 
				equiped: false, }								
	}
    
    pub fn get_item(name: &str) -> Option<Item> {
        match name {
            "longsword" => {
                let mut i = Item::new(name, ItemType::Weapon, 3, false, ')', display::WHITE, display::GREY);
                i.dmg_die = 8;
                Some(i)
            },
            "dagger" => {
                let mut i = Item::new(name, ItemType::Weapon, 1, false, ')', display::WHITE, display::GREY);
                i.dmg_die = 4;
                Some(i)
            },
            "spear" => {
                let mut i = Item::new(name, ItemType::Weapon, 1, false, ')', display::WHITE, display::GREY);
                i.dmg_die = 1;
                Some(i)
            },
            "staff" => {
                let mut i = Item::new(name, ItemType::Weapon, 1, false, ')', display::LIGHT_BROWN, display::BROWN);
                i.dmg_die = 1;
                Some(i)
            },
            "ringmail" => {
                let mut i = Item::new(name, ItemType::Armour, 8, false, '[', display::GREY, display::DARK_GREY);
                i.armour_value = 3;
                Some(i)
            },
            "gold piece" => {
                let i = Item::new(name, ItemType::Zorkmid, 0, true, '$', display::YELLOW_ORANGE, display::GOLD);
                Some(i)
            },
            _ => None,
        }
    }

    pub fn get_full_name(&self) -> String {
		let mut s = String::from(&self.name);

		if self.equiped {
			match self.item_type {
				ItemType::Weapon => s.push_str(" (in hand)"),
				ItemType::Armour => s.push_str(" (being worn)"),
				_ => panic!("Should never hit this option..."),
			}
		}
        
		s
	}
}

impl PartialEq for Item {
	fn eq(&self, other: &Self) -> bool {
		self.name == other.name
	}
}