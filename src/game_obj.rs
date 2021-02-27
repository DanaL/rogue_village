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
use super::{EventType, GameState, PLAYER_INV};
use crate::dialogue::DialogueLibrary;
use crate::items::{Item, ItemType, GoldPile};
use crate::map::Tile;
use crate::player::Player;
use crate::util::StringUtils;

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum GameObjType {
    NPC,
    Item,
    Zorkmids,
}

pub trait GameObject {
    fn blocks(&self) -> bool;
    fn get_location(&self) -> (i32, i32, i8);
    fn set_location(&mut self, loc: (i32, i32, i8));
    fn receive_event(&mut self, event: EventType, state: &mut GameState) -> Option<EventType>;
    fn get_fullname(&self) -> String;
    fn get_object_id(&self) -> usize;
    fn get_type(&self) -> GameObjType;
    fn get_tile(&self) -> Tile;
    fn take_turn(&mut self, state: &mut GameState, game_objs: &mut GameObjects);
    fn is_npc(&self) -> bool;
    fn talk_to(&mut self, state: &mut GameState, player: &Player, dialogue: &DialogueLibrary) -> String;
    // I'm not sure if this is some terrible design sin but sometimes I need to get at the underlying
    // object and didn't want to write a zillion accessor methods. I wonder if I should have gone whole
    // hog down this around and given GameObjets a HashMap of attributes so that I didn't actually need
    // Villager or Item or Zorkminds structs at alll..
    fn as_item(&self) -> Option<Item>;
    fn as_zorkmids(&self) -> Option<GoldPile>;
}

pub struct GameObjects {
    next_obj_id: usize,
    pub obj_locs: HashMap<(i32, i32, i8), VecDeque<usize>>,
    pub objects: HashMap<usize, Box<dyn GameObject>>,
    pub listeners: HashSet<(usize, EventType)>,
    next_slot: char,
}

impl GameObjects {
    pub fn new() -> GameObjects {
        // start at 1 because we assume the player is object 0
        GameObjects { next_obj_id: 1, obj_locs: HashMap::new(), objects: HashMap::new(),
            listeners: HashSet::new(), next_slot: 'a' }
    }

    pub fn add(&mut self, obj: Box<dyn GameObject>) {
        let loc = obj.get_location();
        let obj_id = obj.get_object_id();

        // I want to merge stacks of gold so check the location to see if there 
        // are any there before we insert. For items where there won't be too many of
        // (like torches) that can stack, I don't bother. But storing gold as individual
        // items might have meant 10s of thousands of objects
        if obj.get_type() != GameObjType::Zorkmids || !self.check_for_stack(&obj, loc) {
            self.set_to_loc(obj_id, loc);
            self.objects.insert(obj_id, obj);
        }            
    }

    fn check_for_stack(&mut self, obj: &Box<dyn GameObject>, loc: (i32, i32, i8)) -> bool {
        if self.obj_locs.contains_key(&loc) && self.obj_locs[&loc].len() > 0 {
            let mut pile_id = 0;
            for obj_id in self.obj_locs[&loc].iter() {
                if self.objects[&obj_id].get_type() == GameObjType::Zorkmids {
                    pile_id = *obj_id;
                    break;
                }
            }

            if pile_id > 0 {
                let mut pile = self.get(pile_id).as_zorkmids().unwrap();
                let other = obj.as_zorkmids().unwrap();
                pile.amount += other.amount;
                self.add(Box::new(pile));
                return true;
            }
        }

        false
    }

    pub fn get(&mut self, obj_id: usize) -> Box<dyn GameObject> {
        let obj = self.objects.remove(&obj_id).unwrap();
        let loc = obj.get_location();
        self.remove_from_loc(obj_id, loc);

        obj
    }

    pub fn next_id(&mut self) -> usize {
        let c = self.next_obj_id;
        self.next_obj_id += 1;

        c
    }

    pub fn remove_from_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        let q = self.obj_locs.get_mut(&loc).unwrap();
        q.retain(|v| *v != obj_id);
    }

    fn set_to_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        if !self.obj_locs.contains_key(&loc) {
            self.obj_locs.insert(loc, VecDeque::new());
        }

        self.obj_locs.get_mut(&loc).unwrap().push_front(obj_id);
    }

    pub fn blocking_obj_at(&self, loc: &(i32, i32, i8)) -> bool {
        if self.obj_locs.contains_key(&loc) && self.obj_locs[&loc].len() > 0 {
            for obj_id in self.obj_locs[&loc].iter() {
                if self.objects[&obj_id].blocks() {
                    return true;
                }
            }
        }

        return false;
    }

    pub fn tile_at(&self, loc: &(i32, i32, i8)) -> Option<(Tile, bool)> {
        if self.obj_locs.contains_key(&loc) && self.obj_locs[&loc].len() > 0 {
            for obj_id in self.obj_locs[&loc].iter() {
                if self.objects[&obj_id].blocks() {
                    return Some((self.objects[&obj_id].get_tile(), self.objects[&obj_id].is_npc()));
                }
            }

            let obj_id = self.obj_locs[&loc].front().unwrap();
            return Some((self.objects[obj_id].get_tile(), false));
        }

        None
    }

    pub fn npc_at(&mut self, loc: &(i32, i32, i8)) -> Option<Box<dyn GameObject>> {
        let mut npc_id = 0;

        if let Some(objs) = self.obj_locs.get(loc) {
            for id in objs {
                if self.objects[&id].is_npc() {
                    npc_id = *id;
                    break;                    
                }
            }
        }

        if npc_id > 0 {
            Some(self.get(npc_id))
        } else {
            None
        }        
    }

    pub fn do_npc_turns(&mut self, state: &mut GameState) {
        let actors = self.listeners.iter()
                                   .filter(|i| i.1 == EventType::TakeTurn)
                                   .map(|i| i.0).collect::<Vec<usize>>();

        for actor_id in actors {
            let mut obj = self.get(actor_id);
                        
            obj.take_turn(state, self);

            // There will stuff here that may happen, like if a monster dies while taking
            // its turn, etc
            self.add(obj);
        }
    }

    fn inv_slots_used(&self) -> Vec<(char, Item)> {
        let mut slots = Vec::new();

        if self.obj_locs.contains_key(&PLAYER_INV) && self.obj_locs[&PLAYER_INV].len() > 0 {
            let obj_ids: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();

            for id in obj_ids {
                if let Some(item) = self.objects[&id].as_item() {
                    slots.push((item.slot, item));
                }
            }            
        }

        slots
    }

    pub fn inv_count_at_slot(&self, slot: char) -> u8 {
        if !self.obj_locs.contains_key(&PLAYER_INV) || self.obj_locs[&PLAYER_INV].len() == 0 {
            return 0;
        }

        let items: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();        
        let mut sum = 0;
        for id in items {
            if let Some(item) = self.objects[&id].as_item() {
                if item.slot == slot {
                    sum += 1;
                }
            }
        }     

        sum
    }

    // Caller should check if the slot exists before calling this...
    pub fn inv_remove_from_slot(&mut self, slot: char, amt: u32) -> Result<Vec<Item>, String>  {
        let mut removed = Vec::new();

        let items: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();        
        
        let mut count = 0;
        for id in items {
            if count >= amt {
                break;
            }

            if let Some(item) = self.objects[&id].as_item() {
                if item.slot == slot {
                    if item.equiped && item.item_type == ItemType::Armour {
                        return Err("You're wearing that!".to_string());
                    } else {
                        removed.push(item);
                        self.remove_from_loc(id, PLAYER_INV);
                        count += 1;
                    }
                }
            }
        }     

        Ok(removed)
    }

    pub fn add_to_inventory(&mut self, item: Item) {
        // to add a new item, we
        // check its existing slot. If it's free, great. 
        // If not, if the current item is stackable and can stack with the
        // item there, great!
        // otherwise we find the next available slot and set item's slot to it
        let slots = self.inv_slots_used();
        let used_slots : HashSet<char> = slots.iter()
                                              .map(|s| s.0)
                                              .collect();
                
        if item.stackable() {
            for (_, existing_item) in slots {
                if item == existing_item {
                    let mut i = item.clone();
                    i.slot = existing_item.slot;
                    self.add(Box::new(i));
                    return;                    
                }
            }            
        } 

        let mut i = item.clone();
        if item.slot == '\0' || used_slots.contains(&i.slot) {
            i.slot = self.next_slot;
            
            // Increment the next slot
            let mut slot = self.next_slot;		
            loop {
                slot = (slot as u8 + 1) as char;
                if slot > 'z' {
                    slot = 'a';
                }

                if !used_slots.contains(&slot) {
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

        self.add(Box::new(i));
    }

    pub fn get_inventory_menu(&self) -> Vec<String> {
        let mut menu = Vec::new();
        let slots = self.inv_slots_used();
        let mut used_slots : Vec<char> = slots.iter()
                                              .map(|s| s.0)
                                              .collect();
        used_slots.sort();
        used_slots.dedup();

        let mut menu_items = HashMap::new();
        for s in slots {
            let counter = menu_items.entry(s.0).or_insert((s.1.get_fullname(), 0));
            counter.1 += 1;
        }
        
        for slot in used_slots {
            let mut s = String::from(slot);
            s.push_str(") ");

            let i = menu_items.get(&slot).unwrap();
            if i.1 == 1 {
                s.push_str(&i.0.with_indef_article());
            } else {
                s.push_str(&format!("{} {}", i.1.to_string(), i.0.pluralize()));
            }
            menu.push(s);
        }
        
        menu
    }

    pub fn gear_with_ac_mods(&self) -> Vec<Item> {
        let mut items = Vec::new();

        if self.obj_locs.contains_key(&PLAYER_INV) {
            let ids: Vec<usize> = self.obj_locs[&PLAYER_INV]
                          .iter()
                          .map(|id| *id)
                          .collect();
            for id in ids {
                if let Some(item) = self.objects[&id].as_item() {
                    if item.equiped && item.ac_bonus > 0 {
                        items.push(item);
                    }
                }
            }
        }

        items
    }
    
    pub fn readied_weapon(&self) -> Option<Item> {
        if self.obj_locs.contains_key(&PLAYER_INV) {
            let ids: Vec<usize> = self.obj_locs[&PLAYER_INV]
                          .iter()
                          .map(|id| *id)
                          .collect();
            for id in ids {
                if let Some(item) = self.objects[&id].as_item() {
                    if item.equiped && item.item_type == ItemType::Weapon {
                        return Some(item)
                    }
                }
            }
        }

        None
    }

    // Okay to make life difficult I want to return stackable items described as
    // "X things" instead of having 4 of them in the list
    pub fn descs_at_loc(&self, loc: (i32, i32, i8)) -> Vec<String> {
        let mut v = Vec::new();
        
        let mut items = HashMap::new();
        if self.obj_locs.contains_key(&loc) {
            for j in 0..self.obj_locs[&loc].len() {
                let obj_id = self.obj_locs[&loc][j];
                let name = self.objects[&obj_id].get_fullname();
                let i = items.entry(name).or_insert(0);
                *i += 1;                
            }
        }

        for (key, value) in items {
            if value == 1 {
                let s = format!("{}", key.with_indef_article());
                v.push(s);
            } else {
                let s = format!("{} {}", value, key.pluralize());
                v.push(s);
            }            
        }
        
        v
    }
}

