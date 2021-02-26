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

use std::collections::{HashMap, HashSet, VecDeque};

use crate::display;
use crate::map::Tile;
use crate::util;

// Some bitmasks so that I can store various extra item attributes beyond
// just the item type enum. (Ie., heavy armour, two-handed, etc)
pub const IA_LIGHT_ARMOUR: u32 = 0b00000001;
pub const IA_MED_ARMOUR:   u32 = 0b00000010;
pub const IA_HEAVY_ARMOUR: u32 = 0b00000100;

#[derive(Debug, Clone)]
pub struct Inventory {
	next_slot: char,
	inv: HashMap<char, (Item, u32)>,
    pub purse: u32, // I hope I don't actually need that large an integer for money in this game...
}

impl Inventory {
    pub fn new() -> Inventory {
		Inventory { next_slot: 'a', inv: HashMap::new(), purse: 0 }
	}

    // This doesn't currently handle when all the inventory slots are used up...
    pub fn add(&mut self, item: Item) {
		if item.item_type == ItemType::Zorkmid {
			self.purse += 1;
			return;
		}

		if item.stackable {
			// since the item is stackable, let's see if there's a stack we can add it to
			// Super cool normal programming language way to loop over the keys of a hashtable :/
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

    pub fn count_in_slot(&self, slot: char) -> u32 {
		if !self.inv.contains_key(&slot) {
			0
		} else {
			let v = self.inv.get(&slot).unwrap();
			v.1
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

    // Am I going to allow players to wield non-weapons, a la nethack?
    // This would mean separating the wield command from wear/use, which
    // is a bit more complicated UI for the player.
    pub fn get_readied_weapon(&self) -> Option<Item> {
        let slots = self.used_slots();
        for s in slots {
			let v = self.inv.get(&s).unwrap();
			if v.0.item_type == ItemType::Weapon && v.0.equiped {
				return Some(v.0.clone());
			}
		}

        None
    }

    pub fn peek_at(&self, slot: char) -> Option<Item> {
		if !self.inv.contains_key(&slot) {
			None
		} else {
			let v = self.inv.get(&slot).unwrap();
			Some(v.0.clone())
		}
	}

    // I'm leaving it up to the caller to ensure the slot exists.
	// Bad for a library but maybe okay for my internal game code
	pub fn remove(&mut self, slot: char) -> Item {
		let mut v = self.inv.remove(&slot).unwrap();
		if self.next_slot == '\0' {
			self.next_slot = slot;
		}
		v.0.prev_slot = slot;

		v.0
	}

    pub fn remove_count(&mut self, slot: char, count: u32) -> Vec<Item> {
		let mut items = Vec::new();
		let entry = self.inv.remove_entry(&slot).unwrap();
		let mut v = entry.1;

		let max = if count < v.1 {
			v.1 -= count;
			let replacement = (Item { name: v.0.name.clone(), ..v.0 }, v.1);
			self.inv.insert(slot, replacement);
			count	
		} else {
			if self.next_slot == '\0' {
				self.next_slot = slot;
			}
			v.1
		};

		for _ in 0..max {
			let mut i = Item { name:v.0.name.clone(), ..v.0 }; 
			i.prev_slot = slot;
			items.push(i);
		}

		items
	}

    pub fn toggle_slot(&mut self, slot: char) -> (String, bool) {
		if !self.inv.contains_key(&slot) {
			return (String::from("You do not have that item!"), false);
		}

		let val = self.inv.get_mut(&slot).unwrap();
		let item = val.0.clone();

		if !item.equipable() {
			return (String::from("You cannot equip or use that!"), false);
		}

        if item.item_type == ItemType::Armour && self.type_already_equiped(item.item_type) {
            return (String::from("You are already wearing some armour."), false);
        }

        let swapping = if !item.equiped && item.item_type == ItemType::Weapon && self.get_readied_weapon() != None {
            self.unequip_type(item.item_type);
            true
        } else {
            false
        };

        let item_name = String::from(item.name);

        // Is there a better way to do this?? I'm sticking this in its own little scope
        // because otherwise I get a mutable/immutable borrow conflict when I call to 
        // check if there is a readed weapon after the player toglges their gear
		{
            let val = self.inv.get_mut(&slot).unwrap();
		    let mut item = &mut val.0;        
            item.equiped = !item.equiped;
        }

        let s = if swapping {
            format!("You are now using the {}.", &item_name)
        } else {
            if self.get_readied_weapon() == None {
                String::from("You are now empty handed.")
            } else {
                let mut s = String::from("You ");
                if item.equiped {
                    s.push_str("equip the ");
                } else {
                    s.push_str("unequip the ");
                }
                
                s.push_str(&item_name);
                s.push('.');

                s
            }
        };

		(s, true)
	}
    
    pub fn used_slots(&self) -> Vec<char> {
        self.inv.keys().map(|c| *c).collect()
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

    fn type_already_equiped(&self, i_type: ItemType) -> bool {
		for slot in self.inv.keys() {
			let v = self.inv.get(&slot).unwrap();
			if v.0.item_type == i_type && v.0.equiped {
				return true;
			}
		}

		false
	}

    fn unequip_type(&mut self, i_type: ItemType) {
        let slots = self.used_slots();
        for s in slots {
			let v = self.inv.get_mut(&s).unwrap();
			if v.0.item_type == i_type && v.0.equiped {
				v.0.equiped = false;
			}
		}
    }
}

// In some ways, a simplified version of the inventory struct
// to store items on the ground
#[derive(Debug, Clone)]
pub struct ItemPile {
    pub pile: VecDeque<(Item, u16)>,
}

impl ItemPile {
    pub fn new() -> ItemPile {
        ItemPile { pile: VecDeque::new() }
    }

    pub fn add(&mut self, item: Item) {
        if !item.stackable {
            self.pile.push_front((item, 1));
        } else {
            for i in 0..self.pile.len() {
                if self.pile[i].0 == item {
                    self.pile[i].1 += 1;
                    return;
                }
            }

            self.pile.push_front((item, 1));
        }
    }

	pub fn get(&mut self) -> (Item, u16) {
		self.pile.pop_front().unwrap()
	}

    // Up to the caller to make sure the slot in pile actually exists...
    pub fn get_item_name(&self, nth: usize) -> String {
        if self.pile[nth].1 == 1 {
            let name = self.pile[nth].0.get_full_name();
            let s = format!("{} {}", util::get_indefinite_article(&name), name);
            s
        } else {
            let name = self.pile[nth].0.get_full_name();
            let s = format!("{} {}", self.pile[nth].1, util::pluralize(&name));
            s
        }
    }

	pub fn get_many(&mut self, slots: &HashSet<u8>) -> Vec<(Item, u16)> {
		let mut indices = slots.iter()
								.map(|v| *v as usize)
								.collect::<Vec<usize>>();
		indices.sort();
		indices.reverse();

		let mut items = Vec::new();
		for i in indices {
			if let Some(item) = self.pile.remove(i) {
                items.push(item);
            }
		}

		items
	}

	pub fn get_menu(&self) -> Vec<String> {
		let mut menu = Vec::new();
		
		for j in 0..self.pile.len() {
			let mut s = String::from("");
			s.push(('a' as u8 + j as u8) as char);
			s.push_str(") ");
			if self.pile[j].1 == 1 {
				s.push_str(&self.pile[j].0.name);
			} else {
				s.push_str(&self.pile[j].1.to_string());
				s.push_str(" ");
				s.push_str(&util::pluralize(&self.pile[j].0.name));
			}
			menu.push(s);
		}

		menu
	}

    pub fn get_tile(&self) -> Tile {
        Tile::Thing(self.pile[0].0.lit_colour, self.pile[0].0.unlit_colour, self.pile[0].0.symbol)
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
	pub lit_colour: (u8, u8, u8),
    pub unlit_colour: (u8, u8, u8),
	pub stackable: bool,
	pub prev_slot: char,
	pub dmg_die: u8,
	pub dmg_dice: u8,
	pub attack_bonus: i8,
    pub ac_bonus: i8,
	pub range: u8,
	pub equiped: bool,
    pub attributes: u32,
}

impl Item {    
    fn new(name: &str, item_type: ItemType, weight: u8, stackable: bool, symbol: char, lit_colour: (u8, u8, u8), unlit_colour: (u8, u8, u8)) -> Item {
		Item { name: String::from(name), item_type, weight, stackable, symbol, lit_colour, unlit_colour, prev_slot: '\0',
				dmg_die: 1, dmg_dice: 1, attack_bonus: 0, ac_bonus: 0, range: 0, equiped: false, attributes: 0 }								
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
                i.dmg_die = 6;
                Some(i)
            },
            "staff" => {
                let mut i = Item::new(name, ItemType::Weapon, 1, false, ')', display::LIGHT_BROWN, display::BROWN);
                i.dmg_die = 6;
                Some(i)
            },
            "ringmail" => {
                let mut i = Item::new(name, ItemType::Armour, 8, false, '[', display::GREY, display::DARK_GREY);
                i.ac_bonus = 3;
                i.attributes |= IA_MED_ARMOUR;                
                Some(i)
            },
            "gold piece" => {
                let i = Item::new(name, ItemType::Zorkmid, 0, true, '$', display::GOLD, display::YELLOW_ORANGE);
                Some(i)
            },
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