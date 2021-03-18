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

use sdl2::cpuinfo::system_ram;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use super::{EventResponse, EventType, GameState, PLAYER_INV};
use crate::actor::NPC;
use crate::battle::DamageType;
use crate::items::{Item, GoldPile};
use crate::map::{SpecialSquare, Tile};
use crate::player::Player;
use crate::util::StringUtils;

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

#[derive(Debug)]
pub enum GameObjects {
    Player(Player),
    Item(Item),
    GoldPile(GoldPile),
}

impl GameObject for GameObjects {
    fn blocks(&self) -> bool {
        match self {
            GameObjects::Player(_) => true,
            GameObjects::Item(i) => i.blocks(),
            GameObjects::GoldPile(g) => g.blocks(),
        };

        false
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        match self {
            GameObjects::Player(player) => player.get_loc(),
            GameObjects::Item(i) => i.get_loc(),
            GameObjects::GoldPile(g) => g.get_loc(),
        }
    }

    fn set_loc(&mut self, loc: (i32, i32, i8)) {
        match self {
            GameObjects::Player(player) => player.set_loc(loc),
            GameObjects::Item(i) => i.set_loc(loc),
            GameObjects::GoldPile(g) => g.set_loc(loc),
        }
    }

    fn get_fullname(&self) -> String {
        match self {
            GameObjects::Player(player) => player.get_fullname(),
            GameObjects::Item(i) => i.get_fullname(),
            GameObjects::GoldPile(g) => g.get_fullname(),
        }
    }

    fn obj_id(&self) -> usize {
        match self {
            GameObjects::Player(player) => player.obj_id(),
            GameObjects::Item(i) => i.obj_id(),
            GameObjects::GoldPile(g) => g.obj_id(),
        }
    }

    fn get_tile(&self) -> Tile {
        match self {
            GameObjects::Player(player) => player.get_tile(),
            GameObjects::Item(i) => i.get_tile(),
            GameObjects::GoldPile(g) => g.get_tile(),
        }
    }

    fn hidden(&self) -> bool {
        match self {
            GameObjects::Player(player) => player.hidden(),
            GameObjects::Item(i) => i.hidden(),
            GameObjects::GoldPile(g) => g.hidden(),
        }
    }

    fn hide(&mut self) {
        match self {
            GameObjects::Player(player) => player.hide(),
            GameObjects::Item(i) => i.hide(),
            GameObjects::GoldPile(g) => g.hide(),
        }
    }

    fn reveal(&mut self) {
        match self {
            GameObjects::Player(player) => player.reveal(),
            GameObjects::Item(i) => i.reveal(),
            GameObjects::GoldPile(g) => g.reveal(),
        }
    }

    fn receive_event(&mut self, event: EventType, state: &mut GameState, player_loc: (i32, i32, i8)) -> Option<EventResponse> {
        match self {
            GameObjects::Player(player) => player.receive_event(event, state, player_loc),
            GameObjects::Item(i) => i.receive_event(event, state, player_loc),
            GameObjects::GoldPile(g) => g.receive_event(event, state, player_loc),
        }
    }
}

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

    pub fn set_to_loc(&mut self, obj_id: usize, loc: (i32, i32, i8)) {
        if !self.obj_locs.contains_key(&loc) {
            self.obj_locs.insert(loc, VecDeque::new());
        } 

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
            for obj_id in self.obj_locs[&loc].iter() {                
                if self.objects[&obj_id].blocks() {
                    let remember = if let GameObjects::Player(_) = self.objects[&obj_id] {
                        false
                    //} else if let GameObjects::NPC(_) = self.objects[&obj_id] {
                    //    false
                    } else {
                        true
                    };

                    return Some((self.objects[&obj_id].get_tile(), remember));
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

    pub fn blocking_obj_at(&self, loc: &(i32, i32, i8)) -> bool {
        if self.obj_locs.contains_key(&loc) && !self.obj_locs[&loc].is_empty() {
            for obj_id in self.obj_locs[&loc].iter() {
                if !self.objects.contains_key(obj_id) {
                    let s = format!("Should find obj_id {}!", obj_id);
                    panic!(s);
                }
                if self.objects[&obj_id].blocks() {
                    return true;
                }
            }
        }

        return false;
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

// Any sort of entity that has HPs, feelings, career ambitions... (ie., the Player and the NPCs)
pub trait Person {
    fn damaged(&mut self, state: &mut GameState, amount: u8, dmg_type: DamageType, assailant_id: usize, assailant_name: &str);
    fn get_hp(&self) -> (u8, u8);
    fn add_hp(&mut self, state: &mut GameState, amt: u8);
}

#[derive(Debug)]
pub struct XGameObject {
    pub object_id: usize,
    pub location: (i32, i32, i8),
    pub hidden: bool,
    pub symbol: char,
	pub lit_colour: (u8, u8, u8),
    pub unlit_colour: (u8, u8, u8),
    pub npc: Option<NPC>,
    pub item: Option<Item>,
    //pub gold_pile: Option<GoldPile>,
    pub special_sq: Option<SpecialSquare>,
    pub player: Option<Player>,
    blocks: bool,
    pub name: String,
}

impl XGameObject {
    pub fn new(object_id: usize, name: &str, location: (i32, i32, i8), symbol: char, lit_colour: (u8, u8, u8), 
        unlit_colour: (u8, u8, u8), npc: Option<NPC>, item: Option<Item>, 
            special_sq: Option<SpecialSquare>, player: Option<Player>, blocks: bool) -> Self {
            XGameObject { object_id, name: String::from(name), location, hidden: false, symbol, lit_colour, 
                unlit_colour, npc, item, special_sq, player, blocks, }
        }

    pub fn blocks(&self) -> bool {
        self.blocks
    }

    pub fn set_loc(&mut self, loc: (i32, i32, i8)) {
        self.location = loc;
    }

    pub fn get_fullname(&self) -> String {
        // if self.item.is_some() {
        //     let s = format!("{} {}", self.name, self.item.as_ref().unwrap().desc());
        //     s.trim().to_string()
        // } else if self.gold_pile.is_some() {
        //     self.gold_pile.as_ref().unwrap().get_fullname()
        // } else {
        //     self.name.clone()
        // }
        "".to_string()
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

    fn receive_event(&mut self, event: EventType, state: &mut GameState, player_loc: (i32, i32, i8)) -> Option<EventResponse> {
        // if self.item.is_some() {
        //     self.item.as_mut().unwrap().receive_event(event, state, self.location, player_loc, self.name.clone(), self.object_id)
        // } else if self.special_sq.is_some() {
        //     self.special_sq.as_mut().unwrap().receive_event(event, state, self.location, self.object_id)
        // } else {
        //     None
        // }
        None
    }
}

#[derive(Debug)]
pub struct XGameObjects {
    next_obj_id: usize,
    pub obj_locs: HashMap<(i32, i32, i8), VecDeque<usize>>,
    pub objects: HashMap<usize, XGameObject>,
    pub listeners: HashSet<(usize, EventType)>,
}

impl XGameObjects {
    pub fn new() -> XGameObjects {
        // start at 1 because we assume the player is object 0
        XGameObjects { next_obj_id: 1, obj_locs: HashMap::new(), objects: HashMap::new(),
            listeners: HashSet::new(), }
    }

    pub fn next_id(&mut self) -> usize {
        let c = self.next_obj_id;
        self.next_obj_id += 1;

        c
    }

    pub fn add(&mut self, obj: XGameObject) {
        let loc = obj.location;
        let obj_id = obj.get_object_id();

        // I want to merge stacks of gold so check the location to see if there 
        // are any there before we insert. For items where there won't be too many of
        // (like torches) that can stack, I don't bother. But storing gold as individual
        // items might have meant 10s of thousands of objects
        // if obj.gold_pile.is_some() && self.obj_locs.contains_key(&loc) {
        //     let amt = obj.gold_pile.as_ref().unwrap().amount;
        //     let ids: Vec<usize> = self.obj_locs[&loc].iter().map(|i| *i).collect();
        //     for id in ids {
        //         let obj = self.get_mut(id).unwrap();
        //         if obj.gold_pile.is_some() {
        //             obj.gold_pile.as_mut().unwrap().amount += amt;
        //             return;
        //         }
        //     }            
        // }

        self.set_to_loc(obj_id, loc);
        self.objects.insert(obj_id, obj);        
    }

    pub fn get(&self, obj_id: usize) -> Option<&XGameObject> {
        if !self.objects.contains_key(&obj_id) {
            None
        } else {
            self.objects.get(&obj_id)
        }
    }

    pub fn get_mut(&mut self, obj_id: usize) -> Option<&mut XGameObject> {
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

    pub fn remove(&mut self, obj_id: usize) -> XGameObject {  
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
                if !self.objects.contains_key(obj_id) {
                    let s = format!("Should find obj_id {}!", obj_id);
                    panic!(s);
                }
                if self.objects[&obj_id].blocks() {
                    return true;
                }
            }
        }

        return false;
    }

    pub fn update_listeners(&mut self, state: &mut GameState, event_type: EventType) {
        let ploc = self.player_location();
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
                match obj.receive_event(event_type, state, ploc) {
                    Some(response) => {
                        if response.event_type == EventType::LightExpired {
                            to_remove.push((obj_id, false));                        
                        }
                    },
                    _ => { },
                }
            } else {
                let p = self.player_details();
                if let Some(obj) = p.inv_obj_of_id(obj_id) {
                    //obj.location = PLAYER_INV;
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
                let p = self.player_details();
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

    pub fn stepped_on_event(&mut self, state: &mut GameState, loc: (i32, i32, i8)) {
        let ploc = self.player_location();

        let listeners: Vec<usize> = self.listeners.iter()
            .filter(|l| l.1 == EventType::SteppedOn)
            .map(|l| l.0).collect();

        for obj_id in listeners {
            let obj = self.get_mut(obj_id).unwrap();
            if obj.location == loc {
                if let Some(result) = obj.receive_event(EventType::SteppedOn, state, ploc) {
                    match result.event_type {
                        EventType::TrapRevealed => {
                            let target = self.objects.get_mut(&obj_id).unwrap();
                        target.hidden = false;
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

    pub fn drop_npc_inventory(&mut self, npc: &mut NPC, loc: (i32, i32, i8)) {
        while !npc.inventory.is_empty() {
            let mut obj = npc.inventory.remove(0);
            obj.location = loc;
            if obj.item.is_some() {
                obj.item.as_mut().unwrap().equiped = false;
            }
            self.add(obj);
        }
    }

    pub fn check_for_dead_npcs(&mut self) {
        let ids: Vec<usize> = self.objects.keys().map(|k| *k).collect();

        for id in ids {
            if self.objects[&id].npc.is_some() && !self.objects[&id].npc.as_ref().unwrap().alive {
                let loc = self.objects[&id].location;
                let mut npc = self.remove(id);
                let npc_details = npc.npc.as_mut().unwrap();
                self.drop_npc_inventory(npc_details, loc);
            }
        }
    }

    pub fn do_npc_turns(&mut self, state: &mut GameState) {
        let actors = self.listeners.iter()
                                   .filter(|i| i.1 == EventType::TakeTurn)
                                   .map(|i| i.0).collect::<Vec<usize>>();
        
        for actor_id in actors {            
            // Okay, so I need (or at any rate it's *super* convenient) to pass game_objs int othe take_turns()
            // function for the NPC. (For things like check if squares they want to move to are occupied, etc).
            // But the simplest way to do that I could think of is to remove the NPC GameObject from objects
            // so that there isn't a mutual borrow situation going on. But we gotta remember to re-add it after
            // the NPC's turn is over. (Of course if the diey or something we don't have to).
            
            // NB: I attempted to convert this mess to the objects table holding Rc<RefCell<GameObject>> so that I
            // could multiple ownership and it was a bit of a disaster, but maybe something to try again another time...
            
            // I'm not going to remove them from listeners or obj_locs tables, although we'll have to check if their 
            // position changed after their turn.
            let mut actor = self.objects.remove(&actor_id).unwrap();
            let actor_loc = actor.location;

            // Has the npc died since their last turn?
            let still_alive = actor.npc.as_ref().unwrap().alive;
            if !still_alive {
                self.listeners.retain(|l| l.0 != actor_id);
                self.remove_from_loc(actor_id, actor_loc);
                let actor_details = actor.npc.as_mut().unwrap();
                self.drop_npc_inventory(actor_details, actor_loc);
                continue;    
            }
            
            actor.npc.as_mut().unwrap().curr_loc = actor_loc;
            
            // I don't want to have every single monster in the game taking a turn every round, so
            // only update monsters on the surface or on the same level as the player. (Who knows, in
            // the end maybe it'll be fast enough to always update 100s of monsters..)
            let curr_dungeon_level =  self.player_location().2;      
            if actor_loc.2 == 0 || actor_loc.2 == curr_dungeon_level {
                actor.npc.as_mut().unwrap().take_turn(actor_id, state, self, actor_loc);
            }

            // Was the npc killed during their turn?
            let still_alive = actor.npc.as_ref().unwrap().alive;
            if !still_alive {
                self.listeners.retain(|l| l.0 != actor_id);
                self.remove_from_loc(actor_id, actor_loc);
                let npc_details = actor.npc.as_mut().unwrap();
                self.drop_npc_inventory(npc_details, actor_loc);
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

    pub fn get_pickup_menu(&self, loc: (i32, i32, i8)) -> Vec<(String, usize)> {
        let mut menu = Vec::new();

        if self.obj_locs.contains_key(&loc) {
            let obj_ids = self.obj_locs[&loc].iter().copied();
            for id in obj_ids {
                if self.objects[&id].hidden 
                        || self.objects[&id].special_sq.is_some() 
                        || self.objects[&id].player.is_some() {
                    continue;
                }
                // if self.objects[&id].gold_pile.is_some() {
                //     let amt = self.objects[&id].gold_pile.as_ref().unwrap().amount;
                //     let s = format!("{} gold pieces", amt);
                //     menu.push((s, id));
                // } else {
                //     menu.push((self.objects[&id].get_fullname().with_indef_article(), id));
                // }
            }            
        }

        menu
    }

    pub fn things_at_loc(&self, loc: (i32, i32, i8)) -> Vec<usize> {
        if self.obj_locs.contains_key(&loc) {
            let ids = self.obj_locs[&loc]
                          .iter().copied();
            
            ids.filter(|id| !self.objects[&id].hidden 
                     && self.objects[&id].special_sq.is_none()
                     && self.objects[&id].player.is_none()).collect()
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

    pub fn special_sqs_at_loc(&self, loc: &(i32, i32, i8)) -> Vec<&XGameObject> {
        if self.obj_locs.contains_key(&loc) {
            let ids = self.obj_locs[&loc]
                .iter().copied();

            let specials: Vec<&XGameObject> = ids.filter(|id| self.objects[&id].special_sq.is_some())
                .map(|id| self.objects.get(&id).unwrap())
                .collect();
            specials
        } else {
            Vec::new()
        }
    }
}
