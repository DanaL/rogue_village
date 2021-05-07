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
use crate::battle::DamageType;
use crate::effects;
use crate::items::{Item, GoldPile};
use crate::map::{SpecialSquare, Tile};
use crate::npc;
use crate::npc::NPC;
use crate::player::Player;
use crate::util::StringUtils;
use crate::items;
use crate::items::ItemType;

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum Ability {
    Str,
    Dex,
    Con,
    Chr,
    Apt,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameObjectBase {
    pub object_id: usize,
    pub location: (i32, i32, i8),
    pub hidden: bool,
    pub symbol: char,
	pub lit_colour: (u8, u8, u8),
    pub unlit_colour: (u8, u8, u8),
    pub blocks: bool,
    pub name: String,
}

impl GameObjectBase {
    pub fn new(object_id: usize, location: (i32, i32, i8), hidden: bool, symbol: char, lit_colour: (u8, u8, u8),
                    unlit_colour: (u8, u8, u8), blocks: bool, name: &str) -> GameObjectBase {
        GameObjectBase { object_id, location, hidden, symbol, lit_colour, unlit_colour, blocks, name: String::from(name) }
    }
}

pub trait GameObject {
    fn blocks(&self) -> bool;
    fn get_loc(&self) -> (i32, i32, i8);
    fn set_loc(&mut self, loc: (i32, i32, i8));
    fn get_fullname(&self) -> String;
    fn obj_id(&self) -> usize;
    fn get_tile(&self) -> Tile;
    fn hidden(&self) -> bool;
    fn hide(&mut self);
    fn reveal(&mut self);
    fn receive_event(&mut self, event: EventType, state: &mut GameState, player_loc: (i32, i32, i8)) -> Option<EventResponse>;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum GameObjects {
    Player(Player),
    Item(Item),
    GoldPile(GoldPile),
    NPC(NPC),
    SpecialSquare(SpecialSquare),
}

impl GameObject for GameObjects {
    fn blocks(&self) -> bool {
        match self {
            GameObjects::Player(_) => true,
            GameObjects::Item(i) => i.blocks(),
            GameObjects::GoldPile(g) => g.blocks(),
            GameObjects::NPC(n) => n.blocks(),
            GameObjects::SpecialSquare(sq) => sq.blocks(),
        }
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        match self {
            GameObjects::Player(player) => player.get_loc(),
            GameObjects::Item(i) => i.get_loc(),
            GameObjects::GoldPile(g) => g.get_loc(),
            GameObjects::NPC(n) => n.get_loc(),
            GameObjects::SpecialSquare(sq) => sq.get_loc(),
        }
    }

    fn set_loc(&mut self, loc: (i32, i32, i8)) {
        match self {
            GameObjects::Player(player) => player.set_loc(loc),
            GameObjects::Item(i) => i.set_loc(loc),
            GameObjects::GoldPile(g) => g.set_loc(loc),
            GameObjects::NPC(n) => n.set_loc(loc),
            GameObjects::SpecialSquare(sq) => sq.set_loc(loc),
        }
    }

    fn get_fullname(&self) -> String {
        match self {
            GameObjects::Player(player) => player.get_fullname(),
            GameObjects::Item(i) => i.get_fullname(),
            GameObjects::GoldPile(g) => g.get_fullname(),
            GameObjects::NPC(n) => n.get_fullname(),
            GameObjects::SpecialSquare(sq) => sq.get_fullname(),
        }
    }

    fn obj_id(&self) -> usize {
        match self {
            GameObjects::Player(player) => player.obj_id(),
            GameObjects::Item(i) => i.obj_id(),
            GameObjects::GoldPile(g) => g.obj_id(),
            GameObjects::NPC(n) => n.obj_id(),
            GameObjects::SpecialSquare(sq) => sq.obj_id(),
        }
    }

    fn get_tile(&self) -> Tile {
        match self {
            GameObjects::Player(player) => player.get_tile(),
            GameObjects::Item(i) => i.get_tile(),
            GameObjects::GoldPile(g) => g.get_tile(),
            GameObjects::NPC(n) => n.get_tile(),
            GameObjects::SpecialSquare(sq) => sq.get_tile(),
        }
    }

    fn hidden(&self) -> bool {
        match self {
            GameObjects::Player(player) => player.hidden(),
            GameObjects::Item(i) => i.hidden(),
            GameObjects::GoldPile(g) => g.hidden(),
            GameObjects::NPC(n) => n.hidden(),
            GameObjects::SpecialSquare(sq) => sq.hidden(),
        }
    }

    fn hide(&mut self) {
        match self {
            GameObjects::Player(player) => player.hide(),
            GameObjects::Item(i) => i.hide(),
            GameObjects::GoldPile(g) => g.hide(),
            GameObjects::NPC(n) => n.hide(),
            GameObjects::SpecialSquare(sq) => sq.hide(),
        }
    }

    fn reveal(&mut self) {
        match self {
            GameObjects::Player(player) => player.reveal(),
            GameObjects::Item(i) => i.reveal(),
            GameObjects::GoldPile(g) => g.reveal(),
            GameObjects::NPC(n) => n.reveal(),
            GameObjects::SpecialSquare(sq) => sq.reveal(),
        }
    }

    fn receive_event(&mut self, event: EventType, state: &mut GameState, player_loc: (i32, i32, i8)) -> Option<EventResponse> {
        match self {
            GameObjects::Player(player) => player.receive_event(event, state, player_loc),
            GameObjects::Item(i) => i.receive_event(event, state, player_loc),
            GameObjects::GoldPile(g) => g.receive_event(event, state, player_loc),
            GameObjects::NPC(n) => n.receive_event(event, state, player_loc),
            GameObjects::SpecialSquare(sq) => sq.receive_event(event, state, player_loc),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameObjectDB {
    next_obj_id: usize,
    pub obj_locs: HashMap<(i32, i32, i8), VecDeque<usize>>,
    pub objects: HashMap<usize, GameObjects>,
    pub listeners: HashSet<(usize, EventType)>,
}

impl GameObjectDB {
    pub fn new() -> GameObjectDB {
        // start at 1 because we assume the player is object 0
        GameObjectDB { next_obj_id: 1, obj_locs: HashMap::new(), objects: HashMap::new(),
            listeners: HashSet::new(), }
    }

    pub fn next_id(&mut self) -> usize {
        let c = self.next_obj_id;
        self.next_obj_id += 1;

        c
    }

    pub fn get(&self, obj_id: usize) -> Option<&GameObjects> {
        if !self.objects.contains_key(&obj_id) {
            None
        } else {
            self.objects.get(&obj_id)
        }
    }

    pub fn get_mut(&mut self, obj_id: usize) -> Option<&mut GameObjects> {
        if !self.objects.contains_key(&obj_id) {
            None
        } else {
            self.objects.get_mut(&obj_id)
        }
    }

    pub fn add(&mut self, obj: GameObjects) {
        let loc = obj.get_loc();
        let obj_id = obj.obj_id();

        // I want to merge stacks of gold so check the location to see if there 
        // are any there before we insert. For items where there won't be too many of
        // (like torches) that can stack, I don't bother. But storing gold as individual
        // items might have meant 10s of thousands of objects
        if let GameObjects::GoldPile(zorkmids) = &obj {
            if self.obj_locs.contains_key(&loc) {
                let amt = zorkmids.amount;
                let ids: Vec<usize> = self.obj_locs[&loc].iter().map(|i| *i).collect();
                for id in ids {
                    let obj = self.get_mut(id).unwrap();
                    if let GameObjects::GoldPile(other) = obj {
                        other.amount += amt;
                        return;
                    }                    
                }            
            }
        }
                
        self.set_to_loc(obj_id, loc);
        self.objects.insert(obj_id, obj);        
    }

    pub fn remove(&mut self, obj_id: usize) -> GameObjects {  
        let obj = self.objects.get(&obj_id).unwrap();
        let loc = obj.get_loc();

        self.listeners.retain(|l| l.0 != obj_id);
        self.remove_from_loc(obj_id, loc);
        self.objects.remove(&obj_id).unwrap()        
    }

    pub fn set_to_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        self.obj_locs.entry(loc).or_insert(VecDeque::new());

        self.obj_locs.get_mut(&loc).unwrap().push_front(obj_id);
        if self.objects.contains_key(&obj_id) {
            let prev_loc = self.objects[&obj_id].get_loc();
            self.remove_from_loc(obj_id, prev_loc);
            self.objects.get_mut(&obj_id).unwrap().set_loc(loc);
        }
    }

    pub fn remove_from_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        let q = self.obj_locs.get_mut(&loc).unwrap();
        q.retain(|v| *v != obj_id);
    }

    // Select which tile should be should be on top, ie., which one is shown
    // to the player. The bool in the tuple returned is whether or not the tile should
    // be remembered. Items should be but not the player or the NPCs since they will move
    // around anyhow.
    pub fn tile_at(&self, loc: &(i32, i32, i8)) -> Option<(Tile, bool)> {
        if self.obj_locs.contains_key(&loc) && !self.obj_locs[&loc].is_empty() {
            // Ensure the player or a monster occupying a square is displayed in 
            // preference to items on the square. Check for them first
            for obj_id in self.obj_locs[&loc].iter() {
                if let GameObjects::Player(_) = self.objects[&obj_id] {
                    return Some((self.objects[&obj_id].get_tile(), false));
                }
            }

            for obj_id in self.obj_locs[&loc].iter() {
                if let GameObjects::NPC(npc) = &self.objects[&obj_id] {
                    if !npc.hidden() {
                        return Some((self.objects[&obj_id].get_tile(), false));
                    }
                }
            }

            for obj_id in self.obj_locs[&loc].iter() {
                if !self.objects[obj_id].hidden() {
                    return Some((self.objects[&obj_id].get_tile(), true));
                }
            }
        }

        None
    }

    pub fn blocking_obj_at(&self, loc: &(i32, i32, i8)) -> bool {
        if self.obj_locs.contains_key(&loc) && !self.obj_locs[&loc].is_empty() {
            for obj_id in self.obj_locs[&loc].iter() {
                if !self.objects.contains_key(obj_id) {
                    panic!("{}", format!("Should find obj_id {}!", obj_id));
                }
                if self.objects[&obj_id].blocks() {
                    return true;
                }
            }
        }

        false
    }

    pub fn stepped_on_event(&mut self, state: &mut GameState, loc: (i32, i32, i8)) {
        let ploc = self.objects[&0].get_loc();

        let listeners: Vec<usize> = self.listeners.iter()
            .filter(|l| l.1 == EventType::SteppedOn)
            .map(|l| l.0).collect();

        for obj_id in listeners {
            let obj = self.get_mut(obj_id).unwrap();
            if obj.get_loc() == loc {
                if let Some(result) = obj.receive_event(EventType::SteppedOn, state, ploc) {
                    match result.event_type {
                        EventType::TrapRevealed => {
                            let target = self.objects.get_mut(&obj_id).unwrap();
                            target.reveal();
                        },
                        EventType::Triggered => {                            
                            let target = self.objects.get_mut(&obj_id).unwrap();
                            target.receive_event(EventType::Triggered, state, ploc);
                        },
                        _ => { /* Should maybe panic! here? */ },
                    }
                }
            }
        }
    }

    pub fn player(&mut self) -> Option<&mut Player> {
        if let Some(GameObjects::Player(p)) = self.get_mut(0) {
            Some(p)
        } else {
            None
        }
    }

    pub fn npc(&mut self, obj_id: usize) -> Option<&mut NPC> {
        if let Some(GameObjects::NPC(npc)) = self.get_mut(obj_id) {
            Some(npc)
        } else {
            None
        }
    }

    pub fn person_at(&mut self, loc: (i32, i32, i8)) -> Option<usize> {
        if !self.obj_locs.contains_key(&loc) {
            return None;
        }

        for id in self.obj_locs[&loc].iter() {
            match self.get(*id).unwrap() {
                GameObjects::Player(_) => { return Some(*id); },
                GameObjects::NPC(_) => { return Some(*id); },
                _ => { },
            }            
        }

        None
    }

    pub fn as_person(&mut self, obj_id: usize) -> Option<&mut dyn Person> {
        match self.get_mut(obj_id).unwrap() {
            GameObjects::Player(p) => Some(p),
            GameObjects::NPC(npc) => Some(npc),
            _ => None,
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
                if obj_id == 0 || self.objects[&obj_id].hidden() {
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
            } else if key == "web" {
                    v.push("some webbing".to_string());
            } else {
                let s = format!("{} {}", value, key.pluralize());
                v.push(s);
            }            
        }
        
        v
    }

    pub fn obstacles_at_loc(&self, loc: (i32, i32, i8)) -> Vec<&Item> {
        let mut obstacles = Vec::new();

        if self.obj_locs.contains_key(&loc) {
            for id in self.obj_locs[&loc].iter() {
                if let GameObjects::Item(item) = &self.objects[&id] {
                    if ItemType::Obstacle == item.item_type {
                        obstacles.push(item);
                    }
                }
            }
        }

        obstacles
    }

    pub fn hidden_at_loc(&self, loc: (i32, i32, i8)) -> Vec<usize> {
        if self.obj_locs.contains_key(&loc) {
            let ids = self.obj_locs[&loc]
                .iter().copied();
            
            ids.filter(|id| self.objects[&id].hidden()).collect()
        } else {
            Vec::new()
        }
    }

    pub fn items_to_pick_up(&self, loc: (i32, i32, i8)) -> Vec<usize> {
        if self.obj_locs.contains_key(&loc) {
            let mut ids = Vec::new();
            for id in self.obj_locs[&loc].iter() {
                if *id == 0 || self.objects[&id].hidden() { continue; }
                match &self.objects[&id] {
                    GameObjects::Item(item) => {
                        if item.attributes & items::IA_IMMOBILE == 0 {
                            ids.push(*id);
                        }
                    },
                    GameObjects::GoldPile(_) => ids.push(*id),
                    _ => { continue; },
                }                
            }
            
            ids
        } else {
            Vec::new()
        }
    }

    pub fn things_at_loc(&self, loc: (i32, i32, i8)) -> Vec<usize> {
        if self.obj_locs.contains_key(&loc) {
            let mut ids = Vec::new();
            for id in self.obj_locs[&loc].iter() {
                if *id == 0 || self.objects[&id].hidden() { continue; }
                if let GameObjects::SpecialSquare(_) = self.objects[&id] {
                    continue;
                }

                ids.push(*id);
            }
            
            ids
        } else {
            Vec::new()
        }
    }

    pub fn get_pickup_menu(&self, loc: (i32, i32, i8)) -> Vec<(String, usize)> {
        let mut menu = Vec::new();

        if self.obj_locs.contains_key(&loc) {
            let obj_ids = self.obj_locs[&loc].iter().copied();
            for id in obj_ids {
                if id == 0 || self.objects[&id].hidden() { continue; }
                if let GameObjects::SpecialSquare(_) = self.objects[&id] {
                    continue;
                }
                if let GameObjects::Item(i) = &self.objects[&id] {
                    if i.attributes & items::IA_IMMOBILE > 0 {
                        continue;
                    }
                }

                if let GameObjects::GoldPile(zorkmids) = &self.objects[&id] {
                    let amt = zorkmids.amount;
                    let s = format!("{} gold pieces", amt);
                    menu.push((s, id));
                } else {
                    menu.push((self.objects[&id].get_fullname().with_indef_article(), id));
                }
            }            
        }

        menu
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
                if let GameObjects::NPC(_) = &self.objects[&id] {                
                    return Some(*id);
                }
            }
        }

        None
    }

    pub fn update_listeners(&mut self, state: &mut GameState, event_type: EventType) {
        let ploc = self.get(0).unwrap().get_loc();
        let listeners: Vec<usize> = self.listeners.iter()
            .filter(|l| l.1 == event_type)
            .map(|l| l.0).collect();

        let mut to_remove = Vec::new();

        if event_type == EventType::Update {
            state.lit_sqs.clear();
            state.aura_sqs.clear();
        }
        for obj_id in listeners {
            // Awkward because some items might be on the floor in the dungeon
            // (and thus in the GameObjs structure) while some mgiht be in the player's
            // inventory.
            if let Some(obj) = self.get_mut(obj_id) {        
                if let Some(response) = obj.receive_event(event_type, state, ploc) {
                    if response.event_type == EventType::LightExpired {
                        to_remove.push((obj_id, false));                        
                    }
                }                
            } else {
                let p = self.player().unwrap();
                if let Some(obj) = p.inv_obj_of_id(obj_id) {
                    obj.set_loc(PLAYER_INV);
                    match obj.receive_event(event_type, state, ploc) {
                        Some(response) => {
                            if response.event_type == EventType::LightExpired {
                                to_remove.push((obj_id, true));                        
                            }
                        },
                        _ => { },
                    }
                }
            }
        }

        for item in to_remove {
            if item.1 {
                let p = self.player().unwrap();
                p.inv_remove(item.0);
            } else {
                self.remove(item.0);
            }
        }

        // Now that we've updated which squares are lit, let any listeners know
        let listeners: Vec<usize> = self.listeners.iter()
            .filter(|l| l.1 == EventType::LitUp)
            .map(|l| l.0).collect();

        for obj_id in listeners {
            let obj = self.get_mut(obj_id).unwrap();
            obj.receive_event(EventType::LitUp, state, ploc);            
        }
    }

    fn clear_dead_npc(&mut self, npc_id: usize) {
        self.listeners.retain(|l| l.0 != npc_id);
        let mut npc = self.remove(npc_id);
        if let GameObjects::NPC(npc) = &mut npc {                    
            self.drop_npc_inventory(npc);
        }
    }

    pub fn do_npc_turns(&mut self, state: &mut GameState) {
        let player_loc = self.get(0).unwrap().get_loc();
        let npcs = self.listeners.iter()
                        .filter(|i| i.1 == EventType::TakeTurn)
                        .map(|i| i.0).collect::<Vec<usize>>();
        
        for npc_id in npcs {     
            // Okay, so I need (or at any rate it's *super* convenient) to pass game_objs int othe take_turns()
            // function for the NPC. (For things like check if squares they want to move to are occupied, etc).
            // But the simplest way to do that I could think of is to remove the NPC GameObject from objects
            // so that there isn't a mutual borrow situation going on. But we gotta remember to re-add it after
            // the NPC's turn is over. (Of course if the diey or something we don't have to).
            
            // NB: I attempted to convert this mess to the objects table holding Rc<RefCell<GameObject>> so that I
            // could multiple ownership and it was a bit of a disaster, but maybe something to try again another time...
            
            // Got to remove the NPC from the objects table so I don't hit a mutual borrow situation when interacting 
            // with other game objects
            let npc = self.npc(npc_id).unwrap();
            let npc_loc = npc.get_loc();
            
            // Has the npc died since their last turn?
            let is_alive = npc.alive;
            if !is_alive {
                self.clear_dead_npc(npc_id);
                continue;   
            }
            
            // I don't want to have every single monster in the game taking a turn every round, so
            // only update monsters on the surface or on the same level as the player. (Who knows, in
            // the end maybe it'll be fast enough to always update 100s of monsters..)
            let curr_dungeon_level =  player_loc.2;      
            if npc_loc.2 == 0 || npc_loc.2 == curr_dungeon_level {    
                npc::take_turn(npc_id, state, self);                
            }

            // // Was the npc killed during their turn?
            let npc = self.npc(npc_id).unwrap();
            effects::check_statuses(npc, state);
            let is_alive = npc.alive;
            if !is_alive {
                self.clear_dead_npc(npc_id);
            }
        }
    }

    pub fn drop_npc_inventory(&mut self, npc: &mut NPC) {
        if npc.attributes & npc::MA_LEAVE_CORPSE > 0 {
            let mut pieces = npc.get_corpse(self);
            while !pieces.is_empty() {
                let piece = pieces.remove(0);
                self.add(piece);
            }
        }

        let loc = npc.get_loc();
        while !npc.inventory.is_empty() {
            let mut obj = npc.inventory.remove(0);
            obj.set_loc(loc);
            if let GameObjects::Item(item) = &mut obj {
                item.equiped = false;
            }
            self.add(obj);
        }
    }

    pub fn special_sqs_at_loc(&self, loc: &(i32, i32, i8)) -> Vec<&GameObjects> {
        let mut specials = Vec::new();
        if self.obj_locs.contains_key(&loc) {
            let ids = self.obj_locs[&loc]
                .iter().copied();
            
            for id in ids.into_iter() {
                if let GameObjects::SpecialSquare(_) = &self.objects[&id]{
                    specials.push(self.objects.get(&id).unwrap());
                }
            }            
        } 

        specials
    }

    pub fn check_for_dead_npcs(&mut self) {
        let ids: Vec<usize> = self.objects.keys().map(|k| *k).collect();

        let mut corpses = Vec::new();
        for id in ids {
            if let GameObjects::NPC(npc) = &self.objects[&id] {
                if !npc.alive {
                    corpses.push(id);                    
                }
            }            
        }

        for id in corpses.iter() {
            let npc = self.remove(*id);
            if let GameObjects::NPC(mut npc) = npc {                
                self.drop_npc_inventory(&mut npc);
            }            
        }
    }
}

// Any sort of entity that has HPs, feelings, career ambitions... (ie., the Player and the NPCs)
pub trait Person {
    fn damaged(&mut self, state: &mut GameState, amount: u8, dmg_type: DamageType, assailant_id: usize, assailant_name: &str);
    fn get_hp(&self) -> (u8, u8);
    fn add_hp(&mut self, state: &mut GameState, amt: u8);
    fn ability_check(&self, ability: Ability) -> u8;
    fn attributes(&self) -> u128;
    fn size(&self) -> u8;
    fn mark_dead(&mut self);
    fn alive(&self) -> bool;
    fn calc_ac(&mut self);
}
