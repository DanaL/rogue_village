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
use crate::items::{Item, ItemType, GoldPile};
use crate::map::{SpecialSquare, Tile};
use crate::player::Player;
use crate::util::StringUtils;

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
    pub player: Option<Player>,
    blocks: bool,
    pub name: String,
}

impl GameObject {
    pub fn new(object_id: usize, name: &str, location: (i32, i32, i8), symbol: char, lit_colour: (u8, u8, u8), 
        unlit_colour: (u8, u8, u8), npc: Option<NPC>, item: Option<Item>, gold_pile: Option<GoldPile>, 
            special_sq: Option<SpecialSquare>, player: Option<Player>, blocks: bool) -> Self {
            GameObject { object_id, name: String::from(name), location, hidden: false, symbol, lit_colour, 
                unlit_colour, npc, item, gold_pile, special_sq, player, blocks, }
        }

    pub fn blocks(&self) -> bool {
        self.blocks
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

    // NPCs are slightly more complicated because I want to say in places sometimes
    // "Ed the Innkeeper stabs you." vs "The goblin stabs you."
    pub fn get_npc_name(&self, indef: bool) -> String {
        self.npc.as_ref().unwrap().npc_name(indef)
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

    fn receive_event(&mut self, event: EventType, state: &mut GameState) -> Option<EventResponse> {
        if self.item.is_some() {
            self.item.as_mut().unwrap().receive_event(event, state, self.location, self.name.clone(), self.object_id)
        } else if self.special_sq.is_some() {
            self.special_sq.as_mut().unwrap().receive_event(event, state, self.location, self.object_id)
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameObjects {
    next_obj_id: usize,
    pub obj_locs: HashMap<(i32, i32, i8), VecDeque<usize>>,
    pub objects: HashMap<usize, GameObject>,
    pub listeners: HashSet<(usize, EventType)>,
    pub next_slot: char,
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
        let loc = obj.location;
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

    pub fn player_details(&mut self) -> &mut Player {
        self.get_mut(0).unwrap().player.as_mut().unwrap()
    }

    pub fn player_location(&self) -> (i32, i32, i8) {
        self.objects[&0].location
    }

    // Note there can be more than one item in a slot if they are stackable (ie torches).
    // But if the player is just using one of the items in the stack, we don't really care
    // which one it is so just return the first one.
    pub fn obj_id_in_slot(&self, slot: char) -> usize {
        let inv_ids: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();

        for id in inv_ids {
            if self.objects[&id].item.is_some() {
                if self.objects.get(&id).unwrap().item.as_ref().unwrap().slot == slot {
                    return id;
                }
            }
        }

        0
    }

    pub fn count_in_slot(&self, slot: char) -> usize {
        let inv_ids: Vec<usize> = self.obj_locs[&PLAYER_INV].iter().map(|i| *i).collect();

        let mut count = 0;
        for id in inv_ids {
            if self.objects[&id].item.is_some() {
                if self.objects.get(&id).unwrap().item.as_ref().unwrap().slot == slot {
                    count += 1;
                }
            }
        }

        count
    }

    pub fn remove(&mut self, obj_id: usize) -> GameObject {  
        let obj = self.objects.get(&obj_id).unwrap();
        let loc = obj.location;

        self.listeners.retain(|l| l.0 != obj_id);
        self.remove_from_loc(obj_id, loc);
        self.objects.remove(&obj_id).unwrap()        
    }

    pub fn set_to_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        if !self.obj_locs.contains_key(&loc) {
            self.obj_locs.insert(loc, VecDeque::new());
        } 

        self.obj_locs.get_mut(&loc).unwrap().push_front(obj_id);
        if self.objects.contains_key(&obj_id) {
            let prev_loc = self.objects[&obj_id].location;
            self.remove_from_loc(obj_id, prev_loc);
            self.objects.get_mut(&obj_id).unwrap().location = loc;
        }
    }

    pub fn remove_from_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        let q = self.obj_locs.get_mut(&loc).unwrap();
        q.retain(|v| *v != obj_id);
    }

    pub fn blocking_obj_at(&self, loc: &(i32, i32, i8)) -> bool {
        if self.obj_locs.contains_key(&loc) && !self.obj_locs[&loc].is_empty() {
            for obj_id in self.obj_locs[&loc].iter() {
                if self.objects[&obj_id].blocks() {
                    return true;
                }
            }
        }

        return false;
    }

    pub fn update_listeners(&mut self, state: &mut GameState) {
        let listeners: Vec<usize> = self.listeners.iter()
            .filter(|l| l.1 == EventType::EndOfTurn)
            .map(|l| l.0).collect();

        let mut to_remove = Vec::new();
        state.lit_sqs.clear();
        state.aura_sqs.clear();
        for obj_id in listeners {
            let obj = self.get_mut(obj_id).unwrap();

            match obj.receive_event(EventType::EndOfTurn, state) {
                Some(response) => {
                    if response.event_type == EventType::LightExpired {
                        to_remove.push(obj_id);                        
                    }
                },
                _ => { },
            }
        }

        for obj_id in to_remove {
            self.remove(obj_id);
        }

        // Now that we've updated which squares are lit, let any listeners know
        let listeners: Vec<usize> = self.listeners.iter()
            .filter(|l| l.1 == EventType::LitUp)
            .map(|l| l.0).collect();

        for obj_id in listeners {
            let obj = self.get_mut(obj_id).unwrap();
            obj.receive_event(EventType::LitUp, state);            
        }
    }

    pub fn stepped_on_event(&mut self, state: &mut GameState, loc: (i32, i32, i8)) {
        let listeners: Vec<usize> = self.listeners.iter()
            .filter(|l| l.1 == EventType::SteppedOn)
            .map(|l| l.0).collect();

        for obj_id in listeners {
            let obj = self.get_mut(obj_id).unwrap();
            if obj.location == loc {
                if let Some(result) = obj.receive_event(EventType::SteppedOn, state) {
                    match result.event_type {
                        EventType::TrapRevealed => {
                            let target = self.objects.get_mut(&obj_id).unwrap();
                        target.hidden = false;
                        },
                        EventType::Triggered => {
                            let target = self.objects.get_mut(&obj_id).unwrap();
                            target.receive_event(EventType::Triggered, state);
                        },
                        _ => { /* Should maybe panic! here? */ },
                    }
                }                
            }            
        }
    }

    pub fn tile_at(&self, loc: &(i32, i32, i8)) -> Option<(Tile, bool)> {
        if self.obj_locs.contains_key(&loc) && !self.obj_locs[&loc].is_empty() {
            for obj_id in self.obj_locs[&loc].iter() {
                if self.objects[&obj_id].blocks() {
                    return Some((self.objects[&obj_id].get_tile(), 
                        self.objects[&obj_id].npc.is_some() || self.objects[&obj_id].player.is_some()));
                }
            }

            // I think this actually should be looking for the first non-hidden tile?
            let obj_id = self.obj_locs[&loc].front().unwrap();
            if !self.objects[obj_id].hidden() {
                return Some((self.objects[obj_id].get_tile(), false));
            }
        }

        None
    }

    pub fn check_for_dead_npcs(&mut self) {
        let ids: Vec<usize> = self.objects.keys().map(|k| *k).collect();

        for id in ids {
            if self.objects[&id].npc.is_some() && !self.objects[&id].npc.as_ref().unwrap().alive {
                self.remove(id);
            }
        }
    }

    pub fn do_npc_turns(&mut self, state: &mut GameState, player: &mut Player) {
        let actors = self.listeners.iter()
                                   .filter(|i| i.1 == EventType::TakeTurn)
                                   .map(|i| i.0).collect::<Vec<usize>>();
        
        for actor_id in actors {            
            // Okay, so I need (or at any rate it's *super* convenient) to pass game_objs int othe take_turns()
            // function for the NPC. (For things like check if squares they want to move to are occupied, etc).
            // But the simplest way to do that I could think of is to remove the NPC GameObject from objects
            // so that there isn't a mutual borrow situation going on. But we gotta remember to re-add it after
            // the NPC's turn is over. (Of course if the die or something we don't have to).
            // I'm not going to remove them from listeners or obj_locs tables, although we'll have to check if their 
            // position changed after their turn.
            let mut actor = self.objects.remove(&actor_id).unwrap();
            let actor_loc = actor.location;

            // Has the npc died since their last turn?
            let still_alive = actor.npc.as_ref().unwrap().alive;
            if !still_alive {
                self.listeners.retain(|l| l.0 != actor_id);
                self.remove_from_loc(actor_id, actor_loc);
                continue;    
            }
            
            actor.npc.as_mut().unwrap().curr_loc = actor_loc;
            
            // I don't want to have every single monster in the game taking a turn every round, so
            // only update monsters on the surface or on the same level as the player. (Who knows, in
            // the end maybe it'll be fast enough to always update 100s of monsters..)            
            if actor_loc.2 == 0 || actor_loc.2 == state.player_loc.2 {
                actor.npc.as_mut().unwrap().take_turn(actor_id, state, self, actor_loc, player);
            }

            // Was the npc killed during their turn?
            let still_alive = actor.npc.as_ref().unwrap().alive;
            if !still_alive {
                self.listeners.retain(|l| l.0 != actor_id);
                self.remove_from_loc(actor_id, actor_loc);
                continue;    
            }

            if actor.npc.as_ref().unwrap().curr_loc != actor_loc {
                let new_loc = actor.npc.as_ref().unwrap().curr_loc;
                // the NPC moved on their turn, so we need to update them in the obj_locs table and
                // re-insert them into the objects table. 
                self.remove_from_loc(actor_id, actor_loc);
                actor.location = new_loc;
                self.stepped_on_event(state, new_loc);
                self.add(actor);
            } else {
                // the NPC didn't move so we should just have to put them back into the objects table
                self.objects.insert(actor_id, actor);
            }
        }
    }

    pub fn location_occupied(&self, loc: &(i32, i32, i8)) -> bool {
        if !self.obj_locs.contains_key(loc) {
            return false;
        }

        for id in self.obj_locs[loc].iter() {
            if self.objects[&id].blocks() {
                return true;
            }
        }

        false
    }

    pub fn npc_at(&mut self, loc: &(i32, i32, i8)) -> Option<usize> {        
        if let Some(objs) = self.obj_locs.get(loc) {
            for id in objs {
                if self.objects[&id].npc.is_some() {
                    return Some(*id);
                }
            }
        }

        None
    }

    pub fn inv_slots_used(&self) -> Vec<char> {
        let mut slots = Vec::new();

        if self.obj_locs.contains_key(&PLAYER_INV) && !self.obj_locs[&PLAYER_INV].is_empty() {
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
        if !self.obj_locs.contains_key(&PLAYER_INV) || self.obj_locs[&PLAYER_INV].is_empty() {
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
            
            self.inc_slot();            
        }        
    }
    
    pub fn inc_slot(&mut self) {
        let slots = self.inv_slots_used();
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

    pub fn get_inventory_menu(&self) -> Vec<String> {
        let mut items = Vec::new();
        if self.obj_locs.contains_key(&PLAYER_INV) && !self.obj_locs[&PLAYER_INV].is_empty() {
            for id in self.obj_locs[&PLAYER_INV].iter().copied() {
                if let Some(item) = &self.objects[&id].item {
                    let name = self.objects[&id].get_fullname();
                    items.push((item.slot, name));
                }
            }            
        }
        
        let mut menu = Vec::new();
        let mut slots: Vec<char> = items.iter().map(|i| i.0).collect();
        slots.sort_unstable();
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
            for id in self.obj_locs[&PLAYER_INV].iter().copied() {
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

    pub fn readied_items_of_type(&self, item_type: ItemType) -> Vec<usize> {
        let mut ids = Vec::new();

        for id in self.obj_locs[&PLAYER_INV].iter() {
            let obj = self.objects.get(&id).unwrap();
            if obj.item.is_some() && obj.item.as_ref().unwrap().item_type == item_type && obj.item.as_ref().unwrap().equiped  {
                ids.push(*id);
            }
        }

        ids
    }

    pub fn readied_weapon(&self) -> Option<(&Item, String)> {
        if self.obj_locs.contains_key(&PLAYER_INV) {
            for id in self.obj_locs[&PLAYER_INV].iter().copied() {
                if let Some(item) = &self.objects[&id].item {
                    if item.equiped && item.item_type == ItemType::Weapon {
                        return Some((item, self.objects[&id].get_fullname()))
                    }
                }
            }
        }

        None
    }

    pub fn get_pickup_menu(&self, loc: (i32, i32, i8)) -> Vec<(String, usize)> {
        let mut menu = Vec::new();

        if self.obj_locs.contains_key(&loc) {
            let obj_ids = self.obj_locs[&loc].iter().copied();
            for id in obj_ids {
                if self.objects[&id].hidden || self.objects[&id].special_sq.is_some() {
                    continue;
                }
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
            let ids = self.obj_locs[&loc]
                          .iter().copied();
            
            ids.filter(|id| !self.objects[&id].hidden 
                     && self.objects[&id].special_sq.is_none()).collect()
        } else {
            Vec::new()
        }
    }

    pub fn hidden_at_loc(&self, loc: (i32, i32, i8)) -> Vec<usize> {
        if self.obj_locs.contains_key(&loc) {
            let ids = self.obj_locs[&loc]
                .iter().copied();
            
            ids.filter(|id| self.objects[&id].hidden).collect()
        } else {
            Vec::new()
        }
    }

    pub fn special_sqs_at_loc(&self, loc: &(i32, i32, i8)) -> Vec<&GameObject> {
        if self.obj_locs.contains_key(&loc) {
            let ids = self.obj_locs[&loc]
                .iter().copied();

            let specials: Vec<&GameObject> = ids.filter(|id| self.objects[&id].special_sq.is_some())
                .map(|id| self.objects.get(&id).unwrap())
                .collect();
            specials
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
                v.push(key.with_indef_article());
            } else {
                let s = format!("{} {}", value, key.pluralize());
                v.push(s);
            }            
        }
        
        v
    }
}
