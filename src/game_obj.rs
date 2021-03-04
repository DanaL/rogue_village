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

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use super::{EventResponse, EventType, GameState, PLAYER_INV};
use crate::actor::NPC;
use crate::dialogue::DialogueLibrary;
use crate::items::{Item, ItemType, GoldPile};
use crate::map::{SpecialSquare, Tile};
use crate::player::Player;
use crate::util::StringUtils;

// I keep feeling like in a lot of ways it would be easier to ditch the concrete
// structs that implement the GameObject trait and just have a GameObject struct
// but the various data each type tracks is so disparate I think THAT would turn into
// a mess. A lot of them could go into a table of attributes but some things are more
// complicated. Like the list of steps an npc has for their current plan, the facts 
// they know. Some of the fields will be numeric, some bool, some char or strings.
// What I really want is proper polymorphism instead of traits, which feel like a 
// huge kludge.
#[derive(Debug, Hash, Eq, PartialEq)]
pub enum GameObjType {
    NPC,
    Item,
    Zorkmids,
    SpecialSquare,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameObject {
    pub object_id: usize,
    pub location: (i32, i32, i8),
    pub hidden: bool,
    pub symbol: char,
	pub lit_colour: (u8, u8, u8),
    pub unlit_colour: (u8, u8, u8),
    pub npc: Option<NPC>,
    pub item: Option<Item>,
    pub gold_pile: Option<GoldPile>,
    pub special_sq: Option<SpecialSquare>,
    blocks: bool,
    pub name: String,
}

impl GameObject {
    pub fn new(object_id: usize, name: &str, location: (i32, i32, i8), symbol: char, lit_colour: (u8, u8, u8), unlit_colour: (u8, u8, u8),
        npc: Option<NPC>, item: Option<Item>, gold_pile: Option<GoldPile>, special_sq: Option<SpecialSquare>, blocks: bool) -> Self {
            GameObject { object_id, name: String::from(name), location, hidden: false, symbol, lit_colour, unlit_colour, npc, item, gold_pile, special_sq, blocks, }
        }

    pub fn blocks(&self) -> bool {
        self.blocks
    }

    pub fn get_loc(&self) -> (i32, i32, i8) {
        self.location
    }

    pub fn set_loc(&mut self, loc: (i32, i32, i8)) {
        self.location = loc;
    }

    pub fn get_fullname(&self) -> String {
        if self.item.is_some() {
            let s = format!("{} {}", self.name, self.item.as_ref().unwrap().desc());
            s.trim().to_string()
        } else if self.gold_pile.is_some() {
            self.gold_pile.as_ref().unwrap().get_fullname()
        } else {
            self.name.clone()
        }
    }

    pub fn get_object_id(&self) -> usize {
        self.object_id
    }

    pub fn get_tile(&self) -> Tile {
        Tile::Thing(self.lit_colour, self.unlit_colour, self.symbol)
    }

    pub fn hidden(&self) -> bool {
        self.hidden
    }

    pub fn hide(&mut self) {
        self.hidden = true
    }

    pub fn reveal(&mut self) {
        self.hidden = false
    }
}

pub struct GameObjects {
    next_obj_id: usize,
    pub obj_locs: HashMap<(i32, i32, i8), VecDeque<usize>>,
    pub objects: HashMap<usize, GameObject>,
    pub listeners: HashSet<(usize, EventType)>,
    next_slot: char,
}

impl GameObjects {
    pub fn new() -> GameObjects {
        // start at 1 because we assume the player is object 0
        GameObjects { next_obj_id: 1, obj_locs: HashMap::new(), objects: HashMap::new(),
            listeners: HashSet::new(), next_slot: 'a' }
    }

    pub fn next_id(&mut self) -> usize {
        let c = self.next_obj_id;
        self.next_obj_id += 1;

        c
    }

    pub fn add(&mut self, obj: GameObject) {
        let loc = obj.get_loc();
        let obj_id = obj.get_object_id();

        // I want to merge stacks of gold so check the location to see if there 
        // are any there before we insert. For items where there won't be too many of
        // (like torches) that can stack, I don't bother. But storing gold as individual
        // items might have meant 10s of thousands of objects
        if obj.gold_pile.is_some() && self.obj_locs.contains_key(&loc) {
            let amt = obj.gold_pile.as_ref().unwrap().amount;
            let ids: Vec<usize> = self.obj_locs[&loc].iter().map(|i| *i).collect();
            for id in ids {
                let obj = self.get_mut(id).unwrap();
                if obj.gold_pile.is_some() {
                    obj.gold_pile.as_mut().unwrap().amount += amt;
                    return;
                }
            }            
        }

        self.set_to_loc(obj_id, loc);
        self.objects.insert(obj_id, obj);        
    }

    pub fn get(&mut self, obj_id: usize) -> Option<&GameObject> {
        if !self.objects.contains_key(&obj_id) {
            None
        } else {
            self.objects.get(&obj_id)
        }
    }

    pub fn get_mut(&mut self, obj_id: usize) -> Option<&mut GameObject> {
        if !self.objects.contains_key(&obj_id) {
            None
        } else {
            self.objects.get_mut(&obj_id)
        }
    }

    pub fn remove(&mut self, obj_id: usize) -> GameObject {  
        let obj = self.objects.get(&obj_id).unwrap();
        let loc = obj.location;

        println!("{:?}", self.obj_locs[&loc]);
        self.remove_from_loc(obj_id, loc);
        println!("{:?}", self.obj_locs[&loc]);
        self.objects.remove(&obj_id).unwrap()        
    }

    pub fn set_to_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        if !self.obj_locs.contains_key(&loc) {
            self.obj_locs.insert(loc, VecDeque::new());
        }

        self.obj_locs.get_mut(&loc).unwrap().push_front(obj_id);
        if self.objects.contains_key(&obj_id) {
            self.objects.get_mut(&obj_id).unwrap().location = loc;
        }
    }

    pub fn remove_from_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        let q = self.obj_locs.get_mut(&loc).unwrap();
        q.retain(|v| *v != obj_id);
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

    // pub fn tile_at(&self, loc: &(i32, i32, i8)) -> Option<(Tile, bool)> {
    //     if self.obj_locs.contains_key(&loc) && self.obj_locs[&loc].len() > 0 {
    //         for obj_id in self.obj_locs[&loc].iter() {
    //             if self.objects[&obj_id].blocks() {
    //                 return Some((self.objects[&obj_id].get_tile(), self.objects[&obj_id].is_npc()));
    //             }
    //         }

    //         let obj_id = self.obj_locs[&loc].front().unwrap();
    //         if !self.objects[obj_id].hidden() {
    //             return Some((self.objects[obj_id].get_tile(), false));
    //         }
    //     }

    //     None
    // }

    // pub fn location_occupied(&self, loc: &(i32, i32, i8)) -> bool {
    //     if !self.obj_locs.contains_key(loc) {
    //         return false;
    //     }

    //     for id in self.obj_locs[loc].iter() {
    //         if self.objects[&id].blocks() {
    //             return true;
    //         }
    //     }

    //     false
    // }

    // pub fn npc_at(&mut self, loc: &(i32, i32, i8)) -> Option<Box<dyn GameObject>> {
    //     let mut npc_id = 0;

    //     if let Some(objs) = self.obj_locs.get(loc) {
    //         for id in objs {
    //             if self.objects[&id].is_npc() {
    //                 npc_id = *id;
    //                 break;                    
    //             }
    //         }
    //     }

    //     if npc_id > 0 {
    //         Some(self.get(npc_id))
    //     } else {
    //         None
    //     }        
    // }

    // pub fn do_npc_turns(&mut self, state: &mut GameState) {
    //     let actors = self.listeners.iter()
    //                                .filter(|i| i.1 == EventType::TakeTurn)
    //                                .map(|i| i.0).collect::<Vec<usize>>();

    //     for actor_id in actors {
    //         let mut obj = self.get(actor_id);
            
    //         // I don't want to have every single monster in the game taking a turn every round, so
    //         // only update monsters on the surface or on the same level as the player. (Who knows, in
    //         // the end maybe it'll be fast enough to always update 100s of monsters..)
    //         let loc = obj.get_location();
    //         if loc.2 == 0 || loc.2 == state.player_loc.2 {
    //             obj.take_turn(state, self);
    //         }

    //         // There will stuff here that may happen, like if a monster dies while taking
    //         // its turn, etc
    //         self.add(obj);
    //     }
    // }

    // pub fn end_of_turn(&mut self, state: &mut GameState) {
    //     let listeners: Vec<usize> = self.listeners.iter()
    //         .filter(|l| l.1 == EventType::EndOfTurn)
    //         .map(|l| l.0).collect();

    //     state.lit_sqs.clear();
    //     state.aura_sqs.clear();
    //     for obj_id in listeners {
    //         let mut obj = self.get(obj_id);

    //         match obj.receive_event(EventType::EndOfTurn, state) {
    //             Some(response) => {
    //                 if response.event_type == EventType::LightExpired {
    //                     self.listeners.remove(&(obj.get_object_id(), EventType::EndOfTurn));
    //                 }
    //             },
    //             _ => self.add(obj),
    //         }
    //     }

    //     // Now that we've updated which squares are lit, let any listeners know
    //     let listeners: Vec<usize> = self.listeners.iter()
    //         .filter(|l| l.1 == EventType::LitUp)
    //         .map(|l| l.0).collect();

    //     for obj_id in listeners {
    //         let mut obj = self.get(obj_id);
    //         obj.receive_event(EventType::LitUp, state);
    //         self.add(obj);
    //     }
    // }

    // pub fn stepped_on_event(&mut self, state: &mut GameState, loc: (i32, i32, i8)) {
    //     let listeners: Vec<usize> = self.listeners.iter()
    //         .filter(|l| l.1 == EventType::SteppedOn)
    //         .map(|l| l.0).collect();

    //     for obj_id in listeners {
    //         let mut obj = self.get(obj_id);
    //         if obj.get_location() == loc {
    //             if let Some(result) = obj.receive_event(EventType::SteppedOn, state) {
    //                 let target = self.objects.get_mut(&result.object_id).unwrap();
    //                 target.receive_event(EventType::Triggered, state);
    //             }
    //         }
    //         self.add(obj);
    //     }
    // }

    pub fn inv_slots_used(&self) -> Vec<char> {
        let mut slots = Vec::new();

        if self.obj_locs.contains_key(&PLAYER_INV) && self.obj_locs[&PLAYER_INV].len() > 0 {
            let obj_ids: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();

            for id in obj_ids {
                if let Some(item) = &self.objects[&id].item {
                    slots.push(item.slot);
                }
            }            
        }

        slots
    }

    pub fn inv_count_at_slot(&self, slot: char) -> u8 {
        if !self.obj_locs.contains_key(&PLAYER_INV) || self.obj_locs[&PLAYER_INV].len() == 0 {
            return 0;
        }

        let ids: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();        
        let mut sum = 0;
        for id in ids {
            if let Some(item) = &self.objects[&id].item {
                if item.slot == slot {
                    sum += 1;
                }
            }
        }     

        sum
    }

    // // Caller should check if the slot exists before calling this...
    pub fn inv_remove_from_slot(&mut self, slot: char, amt: u32) -> Result<Vec<usize>, String>  {
        let mut removed = Vec::new();

        let items: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();        
        
        let mut count = 0;
        for id in items {
            if count >= amt {
                break;
            }

            let obj_id = self.objects[&id].object_id;
            if let Some(item) = &self.objects[&id].item {
                if item.slot == slot {
                    if item.equiped && item.item_type == ItemType::Armour {
                        return Err("You're wearing that!".to_string());
                    } else {
                        removed.push(obj_id);
                        self.remove_from_loc(id, PLAYER_INV);
                        count += 1;
                    }
                }
            }
        }     

        Ok(removed)
    }

    pub fn add_to_inventory(&mut self, mut obj: GameObject) {
        // to add a new item, we
        // check its existing slot. If it's free, great. 
        // If not, if the current item is stackable and can stack with the
        // item there, great!
        // otherwise we find the next available slot and set item's slot to it
        let slots = self.inv_slots_used();
        let obj_id = obj.object_id;

        if obj.item.as_ref().unwrap().stackable() {
            for id in self.obj_locs[&PLAYER_INV].iter() {
                let o = &self.objects[&id];
                let other_item = &o.item.as_ref().unwrap();
                if obj.name == o.name && other_item.stackable {
                    obj.item.as_mut().unwrap().slot = other_item.slot;
                    obj.location = PLAYER_INV;                 
                    self.add(obj);
                    self.objects.get_mut(&obj_id).unwrap().set_loc(PLAYER_INV);
                    return;
                }
            }            
        }

        obj.location = PLAYER_INV;
        self.add(obj);        
        self.objects.get_mut(&obj_id).unwrap().set_loc(PLAYER_INV);
        
        let curr_slot = self.objects[&obj_id].item.as_ref().unwrap().slot;     
        if curr_slot == '\0' || slots.contains(&curr_slot) {
            self.objects.get_mut(&obj_id).unwrap()
                        .item.as_mut().unwrap().slot = self.next_slot;
            
            // Increment the next slot
            let mut nslot = self.next_slot;		
            loop {
                nslot = (nslot as u8 + 1) as char;
                if nslot > 'z' {
                    nslot = 'a';
                }

                if !slots.contains(&nslot) {
                    self.next_slot = nslot;
                    break;
                }

                if nslot == self.next_slot {
                    // No free spaces left in the invetory!
                    self.next_slot = '\0';
                    break;
                }
            }
        }        
    }
    
    pub fn get_inventory_menu(&self) -> Vec<String> {
        let mut items = Vec::new();
        if self.obj_locs.contains_key(&PLAYER_INV) && self.obj_locs[&PLAYER_INV].len() > 0 {
            let obj_ids: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();

            for id in obj_ids {
                if let Some(item) = &self.objects[&id].item {
                    let name = self.objects[&id].get_fullname();
                    items.push((item.slot, name));
                }
            }            
        }
        
        let mut menu = Vec::new();
        let mut slots: Vec<char> = items.iter().map(|i| i.0).collect();
        slots.sort();
        slots.dedup();
        let mut menu_items = HashMap::new();
        for s in items {
            let counter = menu_items.entry(s.0).or_insert((s.1, 0));
            counter.1 += 1;
        }
        
        for slot in slots {
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

    pub fn ac_mods_from_gear(&self) -> (i8, u32) {
        let mut sum = 0;
        let mut attributes = 0;
        if self.obj_locs.contains_key(&PLAYER_INV) {
            let ids: Vec<usize> = self.obj_locs[&PLAYER_INV]
                          .iter()
                          .map(|id| *id)
                          .collect();
                      
            for id in ids {
                if let Some(item) = &self.objects[&id].item {
                    if item.equiped && item.ac_bonus > 0 {
                        sum += item.ac_bonus;
                        attributes |= item.attributes;                 
                    }
                }
            }
        }

        (sum, attributes)
    }
       
    // pub fn readied_armour(&self) -> Option<Item> {
    //     if self.obj_locs.contains_key(&PLAYER_INV) {
    //         let ids: Vec<usize> = self.obj_locs[&PLAYER_INV]
    //                       .iter()
    //                       .map(|id| *id)
    //                       .collect();
    //         for id in ids {
    //             if let Some(item) = self.objects[&id].as_item() {
    //                 if item.equiped && item.item_type == ItemType::Armour {
    //                     return Some(item)
    //                 }
    //             }
    //         }
    //     }

    //     None
    // }

    pub fn readied_weapon(&self) -> String {
        if self.obj_locs.contains_key(&PLAYER_INV) {
            let ids: Vec<usize> = self.obj_locs[&PLAYER_INV]
                          .iter()
                          .map(|id| *id)
                          .collect();
            for id in ids {
                if let Some(item) = &self.objects[&id].item {
                    if item.equiped && item.item_type == ItemType::Weapon {
                        return self.objects[&id].name.clone()
                    }
                }
            }
        }

        "".to_string()
    }

    pub fn get_pickup_menu(&self, loc: (i32, i32, i8)) -> Vec<(String, usize)> {
        let mut menu = Vec::new();

        if self.obj_locs.contains_key(&loc) {
            let obj_ids: Vec<usize> = self.obj_locs[&loc].iter().map(|i| *i).collect();

            for id in obj_ids {
                if self.objects[&id].gold_pile.is_some() {
                    let amt = self.objects[&id].gold_pile.as_ref().unwrap().amount;
                    let s = format!("{} gold pieces", amt);
                    menu.push((s, id));
                } else {
                    menu.push((self.objects[&id].get_fullname().with_indef_article(), id));
                }
            }            
        }

        menu
    }

    pub fn things_at_loc(&self, loc: (i32, i32, i8)) -> Vec<usize> {
        if self.obj_locs.contains_key(&loc) {
            let ids: Vec<usize> = self.obj_locs[&loc]
                          .iter()
                          .map(|id| *id)
                          .collect();
            ids            
        } else {
            Vec::new()
        }
    }

    // Okay to make life difficult I want to return stackable items described as
    // "X things" instead of having 4 of them in the list
    pub fn descs_at_loc(&self, loc: &(i32, i32, i8)) -> Vec<String> {
        let mut v = Vec::new();
        
        let mut items = HashMap::new();
        if self.obj_locs.contains_key(loc) {
            for j in 0..self.obj_locs[&loc].len() {                
                let obj_id = self.obj_locs[&loc][j];
                if self.objects[&obj_id].hidden() {
                    continue;
                }
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

#[derive(Serialize, Deserialize)]
pub struct GOForSerde {
    pub next_obj_id: usize,
    pub villagers: Vec<NPC>,
    pub items: Vec<Item>,
    pub gold_piles: Vec<GoldPile>,
    pub special_sqs: Vec<SpecialSquare>,
    pub listeners: HashSet<(usize, EventType)>,
    pub next_slot: char,
}

impl GOForSerde {
    pub fn convert(game_objs: &GameObjects) -> GOForSerde {
        let mut for_serde = GOForSerde {
            next_obj_id: game_objs.next_obj_id, next_slot: game_objs.next_slot,
            villagers: Vec::new(), items: Vec::new(), gold_piles: Vec::new(),
            listeners: HashSet::new(), special_sqs: Vec::new(),
        };

        for l in game_objs.listeners.iter() {
            for_serde.listeners.insert(*l);
        }

        // for id in game_objs.objects.keys() {
        //     let obj = game_objs.objects.get(id).unwrap();
        //     if let Some(item) = obj.as_item() {
        //         for_serde.items.push(item);
        //     } else if let Some(pile) = obj.as_zorkmids() {
        //         for_serde.gold_piles.push(pile);
        //     } else if let Some(villager) = obj.as_villager() {
        //         for_serde.villagers.push(villager);
        //     } else if let Some(special_sq) = obj.as_special_sq() {
        //         for_serde.special_sqs.push(special_sq);
        //     }
        // }

        for_serde
    }

    pub fn revert(go: GOForSerde) -> GameObjects {
        let mut game_objects = GameObjects::new();
        game_objects.next_slot = go.next_slot;
        game_objects.next_obj_id = go.next_obj_id;

        for l in go.listeners.iter() {
            game_objects.listeners.insert(*l);
        }
        // for v in go.villagers {
        //     game_objects.add(Box::new(v));
        // }
        // for i in go.items {
        //     game_objects.add(Box::new(i));
        // }
        // for g in go.gold_piles {
        //     game_objects.add(Box::new(g));
        // }
        // for sq in go.special_sqs {
        //     game_objects.add(Box::new(sq));
        // }

        game_objects
    }
}

