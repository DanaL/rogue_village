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

use display::YELLOW_ORANGE;

use super::{EventListener, EventType, GameState, GameObject, GameObjects};

use crate::dialogue::DialogueLibrary;
use crate::display;
use crate::map::Tile;
use crate::player::Player;
use crate::util::StringUtils;

// Some bitmasks so that I can store various extra item attributes beyond
// just the item type enum. (Ie., heavy armour, two-handed, etc)
pub const IA_LIGHT_ARMOUR: u32 = 0b00000001;
pub const IA_MED_ARMOUR:   u32 = 0b00000010;
pub const IA_HEAVY_ARMOUR: u32 = 0b00000100;

#[derive(Debug, Clone)]
pub struct Inventory {
	next_slot: char,
	pub inv: HashMap<char, (Item, u32)>,
    pub purse: u32, // I hope I don't actually need that large an integer for money in this game...
}

impl Inventory {
    pub fn new() -> Inventory {
		Inventory { next_slot: 'a', inv: HashMap::new(), purse: 0 }
	}

    // This doesn't currently handle when all the inventory slots are used up...
    // pub fn add(&mut self, item: Item) -> char {
    //     if item.item_type == ItemType::Zorkmid {
	// 		self.purse += 1;
	// 		return '$';
	// 	}

	// 	if item.stackable {
	// 		let slots = self.used_slots();
    //     	for slot in slots {
	// 			let mut val = self.inv.get_mut(&slot).unwrap();
	// 			if val.0 == item && val.0.stackable {
	// 				val.1 += 1;
	// 				return slot;
	// 			}
	// 		}
	// 	} 

	// 	// If the last slot the item occupied is still available, use that
	// 	// instead of the next available slot.
	// 	if item.prev_slot != '\0' && !self.inv.contains_key(&item.prev_slot) {
    //         let s = item.prev_slot;
	// 		self.inv.insert(item.prev_slot, (item, 1));
    //         s
	// 	} else {
    //         let s = self.next_slot;
	// 		self.inv.insert(self.next_slot, (item, 1));
	// 		self.set_next_slot();
    //         s
	// 	}
	// }

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

		// let mut slots = self.inv
		// 	.keys()
		// 	.map(|v| v.clone())
		// 	.collect::<Vec<char>>();
		// slots.sort();

        // if self.purse == 1 {
        //     menu.push(String::from("$) a single zorkmid to your name"));
        // } else if self.purse > 1 {
        //     menu.push(format!("$) {} gold pieces", self.purse));
        // }

		// for slot in slots {
		// 	let mut s = String::from("");
		// 	s.push(slot);
		// 	s.push_str(") ");
		// 	let val = self.inv.get(&slot).unwrap();
		// 	if val.1 == 1 {
		// 		s.push_str("a ");
		// 		s.push_str(&val.0.get_full_name());
		// 	} else {
		// 		s.push_str(&val.0.get_full_name());
		// 		s.push_str(" x");
		// 		s.push_str(&val.1.to_string());
		// 	}
		// 	menu.push(s);
		// }

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

    pub fn get_readied_armour(&self) -> Option<Item> {
        let slots = self.used_slots();
        for s in slots {
			let v = self.inv.get(&s).unwrap();
			if v.0.item_type == ItemType::Armour && v.0.equiped {
				return Some(v.0.clone());
			}
		}

        None
    }

    // Return the highest light radius from among active items in
    // inventory
    pub fn light_from_items(&self) -> u8 {
        let max_aura = self.inv
                    .iter()
                    .filter(|i| i.1.0.active)
                    .map(|i| i.1.0.aura)
                    .max();
        
        if let Some(v) = max_aura {
            v
        } else {
            0
        }

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
	// pub fn remove(&mut self, slot: char) -> Item {
	// 	let mut v = self.inv.remove(&slot).unwrap();
	// 	if self.next_slot == '\0' {
	// 		self.next_slot = slot;
	// 	}
	// 	v.0.prev_slot = slot;

	// 	v.0
	// }

    // pub fn remove_count(&mut self, slot: char, count: u32) -> Vec<Item> {
	// 	let mut items = Vec::new();
	// 	let entry = self.inv.remove_entry(&slot).unwrap();
	// 	let mut v = entry.1;

	// 	let max = if count < v.1 {
	// 		v.1 -= count;
	// 		let replacement = (Item { name: v.0.name.clone(), ..v.0 }, v.1);
	// 		self.inv.insert(slot, replacement);
	// 		count	
	// 	} else {
	// 		if self.next_slot == '\0' {
	// 			self.next_slot = slot;
	// 		}
	// 		v.1
	// 	};

	// 	for _ in 0..max {
	// 		let mut i = Item { name:v.0.name.clone(), ..v.0 }; 
	// 		i.prev_slot = slot;
	// 		items.push(i);
	// 	}

	// 	items
	// }

    // This is pretty simple for now because the only item with an activateable effect are torches
    pub fn use_item_in_slot(&mut self, slot: char, state: &mut GameState) -> String {
        // Check to see if the item is actually useable is assumed done by the caller method
        let val = self.inv.get(&slot).unwrap().clone();
        let item = val.0;
        let stack_count = val.1;

        let s = if item.active { 
            format!("You extinguish {}.", item.name.with_def_article())
        } else {
            format!("{} blazes brightly!", item.name.with_def_article().capitalize())
        };

        // Stackable, equipable items make things slightly complicated. I am assuming any stackble, equipable 
        // thing is basically something like a torch that has charges counting down so remove it from the stack
        if stack_count > 1 && item.stackable {
            //let mut light = Item::get_item(state, &item.name).unwrap();
            //light.active = !item.active;

            // if light.active {
            //     state.listeners.insert((light.object_id, EventType::EndOfTurn));
            // } else {
            //     state.listeners.insert((light.object_id, EventType::EndOfTurn));
            // }

            //self.inv.insert(slot, (item, stack_count -1));
            
            // TODO: handle the case where there is no free inventory slot for the torch that is now
            // separate from the stack            
            //self.add(light);
        } else {
            let val = self.inv.get_mut(&slot).unwrap();
            let mut item = &mut val.0;
            item.active = !item.active;

            // if item.active {
            //     state.listeners.insert((item.object_id, EventType::EndOfTurn));
            // } else {
            //     state.listeners.insert((item.object_id, EventType::EndOfTurn));
            // }
        }

        s
    }

    // pub fn toggle_slot(&mut self, slot: char) -> (String, bool) {
	// 	if !self.inv.contains_key(&slot) {
	// 		return (String::from("You do not have that item!"), false);
	// 	}
        
	// 	let val = self.inv.get(&slot).unwrap().clone();
    //     let item = val.0;
    //     let stack_count = val.1;
	// 	let item_name = item.name.clone();
        
	// 	if !item.equipable() {
	// 		return (String::from("You cannot wear/wield that!"), false);
	// 	}

    //     // Stackable, equipable items make things slightly complicated. I am assuming any stackble, equipable 
    //     // thing is basically something like a torch that has charges counting down so remove it from the stack
    //     if stack_count > 1 && item.stackable {
    //         let mut light = item.clone();
    //         light.equiped = true;

    //         self.inv.insert(slot, (item, stack_count -1));
                        
    //         // TODO: handle the case where there is no free inventory slot for the torch that is now
    //         // separate from the stack            
    //         self.add(light);

    //         let s = format!("The {} blazes brightly!", item_name);

    //         return (s, true);
    //     }

    //     let mut swapping = false;
    //     if item.item_type == ItemType::Weapon {
    //         if let Some(w) = self.get_readied_weapon() {
    //             if w.object_id != item.object_id {
    //                 swapping = true;
    //                 self.unequip_type(item.item_type);
    //             }
    //         }
    //     } else if item.item_type == ItemType::Armour {
    //         if let Some(a) = self.get_readied_armour() {
    //             if a.object_id != item.object_id {
    //                 return (String::from("You are already wearing armour."), false);
    //             }
    //         }
    //     }
        
    //     // Alright, so at this point we can toggle the item in the slot.
    //     let mut item_slot = self.inv.get_mut(&slot).unwrap();
    //     item_slot.0.equiped = !item_slot.0.equiped;

    //     let mut s = String::from("You ");
        
    //     if swapping {
    //         s.push_str("are now wielding ")
    //     } else if item_slot.0.equiped {
    //         s.push_str("equip ");
    //     } else {
    //         s.push_str("unequip ");
    //     }
        
    //     s.push_str(&item_name.with_def_article());
    //     s.push('.');
        
    //     if self.get_readied_weapon() == None {
    //          s = String::from("You are now empty handed.");
    //     } 

	// 	(s, true)        
	// }
    
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

    // // Up to the caller to make sure the slot in pile actually exists...
    // pub fn get_item_name(&self, nth: usize) -> String {
    //     if self.pile[nth].1 == 1 {
    //         let name = self.pile[nth].0.get_full_name();
    //         name.with_indef_article()
    //     } else {
    //         let name = self.pile[nth].0.get_full_name();
    //         let s = format!("{} {}", self.pile[nth].1, name.pluralize());
    //         s
    //     }
    // }

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
				s.push_str(&self.pile[j].0.name.pluralize());
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
    Light,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub object_id: usize,
    pub name: String,
	pub item_type: ItemType,
	pub weight: u8,
	pub symbol: char,
	pub lit_colour: (u8, u8, u8),
    pub unlit_colour: (u8, u8, u8),
	stackable: bool,
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
}

impl Item {    
    fn new(object_id: usize, name: &str, item_type: ItemType, weight: u8, stackable: bool, symbol: char, lit_colour: (u8, u8, u8), unlit_colour: (u8, u8, u8)) -> Item {
		Item { object_id, name: String::from(name), item_type, weight, stackable, symbol, lit_colour, unlit_colour, slot: '\0',
				dmg_die: 1, dmg_dice: 1, attack_bonus: 0, ac_bonus: 0, range: 0, equiped: false, attributes: 0, active: false, charges: 0, aura: 0,
                location: (-1, -1, -1) }								
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
                i.charges = 500;
                i.aura = 5;
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
}

impl EventListener for Item {
    fn receive(&mut self, event: EventType, state: &mut GameState) -> Option<EventType> {
        match event {
            EventType::EndOfTurn => {
                if self.active {
                    self.charges -= 1;
                }

                if self.charges == 0 {
                    let s = format!("{} has gone out!", self.name.with_def_article().capitalize());
                    state.write_msg_buff(&s);
                    Some(EventType::LightExpired)
                } else if self.charges < 100 {
                    let s = format!("{} sputters.", self.name.with_def_article().capitalize());
                    state.write_msg_buff(&s);
                    None
                } else {                    
                    None
                }
            },
            _ => None
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

    fn as_item(&self) -> Option<Item> {
        Some(self.clone())
    }

    fn take_turn(&mut self, state: &mut GameState, game_objs: &mut GameObjects) {
         
    }

    fn talk_to(&mut self, state: &mut GameState, player: &Player, dialogue: &DialogueLibrary) -> String {
        format!("You are trying to talk to {}...", self.get_fullname().with_indef_article())
    }
}
#[derive(Debug, Clone)]
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

    fn receive_event(&mut self, event: EventType, state: &mut GameState) -> Option<EventType> {
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
        Tile::Thing(display::GOLD,  display::YELLOW_ORANGE, '$')
    }

    fn as_item(&self) -> Option<Item> {
        None
    }

    fn take_turn(&mut self, state: &mut GameState, game_objs: &mut GameObjects) {
         
    }

    fn talk_to(&mut self, state: &mut GameState, player: &Player, dialogue: &DialogueLibrary) -> String {
        String::from("You are trying to talk to a pile of money...")
    }
}
