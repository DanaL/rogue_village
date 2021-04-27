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

use std::collections::{HashMap, HashSet};
use rand::Rng;
use serde::{Serialize, Deserialize};

use super::{EventResponse, EventType, GameState, Message, Status};
use crate::battle::DamageType;
use crate::display;
use crate::effects::HasStatuses;
use crate::items;
use crate::game_obj::{Ability, GameObject, GameObjectDB, GameObjectBase, GameObjects, Person};
use crate::items::{Item, ItemType};
use crate::map::Tile;
use crate::util::StringUtils;

const XP_CHART: [u32; 19] = [20, 40, 80, 160, 320, 640, 1280, 2560, 5210, 10_000, 15_000, 21_000, 28_000, 36_000, 44_000, 52_000, 60_000, 68_000, 76_000];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Role {
    Warrior,
    Rogue,
}

impl Role {
    pub fn desc(&self) -> &str {
        match self {
            Role::Warrior => "human warrior",
            Role::Rogue => "human rogue",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Player {
    pub base_info: GameObjectBase,
    pub max_hp: u8,
	pub curr_hp: u8,
	pub vision_radius: u8,
    pub str: u8,
    pub dex: u8,
    pub con: u8,
    pub chr: u8,
    pub apt: u8,
    pub role: Role,
    pub xp: u32,
    pub level: u8,
    pub max_depth: u8,
    pub ac: u8,
    pub purse: u32,
    pub readied_weapon: String,
    pub energy: f32,
    pub energy_restore: f32,
    pub inventory: Vec<GameObjects>,
    pub next_slot: char,
    pub hit_die: u8,
    pub stealth_score: u8,
    pub statuses: Vec<Status>,
    pub size: u8,
}

impl Player {
    pub fn calc_vision_radius(&mut self, state: &mut GameState, loc: (i32, i32, i8)) {
        let prev_vr = self.vision_radius;
        let (hour, _) = state.curr_time();

        if loc.2 == 0 {
            // outdoors
            self.vision_radius = if hour >= 6 && hour <= 19 {
                99
            } else if hour >= 20 && hour <= 21 {
                8
            } else if hour >= 21 && hour <= 23 {
                7
            } else if hour < 4 {
                5
            } else if hour >= 4 && hour < 5 {
                7
            } else {
                9
            };
        } else {
            // indoors
            // Eventually roles who are dwarves, elves, etc will see in the dark better than
            // humans
            self.vision_radius = match self.role {
                Role::Rogue => 1,
                Role::Warrior => 1,
            }
        }

        for status in &self.statuses {
            match status {
                Status::BlindUntil(_) => { self.vision_radius = 0 ;},
                _ => { },
            }
        }

        // Announce sunrise and sunset if the player is on the surface
        // This should be here and is a dumb calculation because vision radius will be
        // affected by say torches. It should be moved to end-of-turn stuff in the gameloop
        if prev_vr == 99 && self.vision_radius == 9 && loc.2 == 0 {
            state.msg_queue.push_back(Message::new(0, loc, "The sun is beginning to set.", ""));            
        }
        if prev_vr == 5 && self.vision_radius == 7 && loc.2 == 0 {
            state.msg_queue.push_back(Message::new(0, loc, "Sunrise soon.", ""));            
        }
    }

    pub fn new_warrior(game_obj_db: &mut GameObjectDB, name: &str) {
        let default_vision_radius = 99;
        let stats = roll_stats();
        
        let mut rng = rand::thread_rng();
        let (chr, apt) = if rng.gen_range(0.0, 1.0) < 0.5 {
            (stats[3], stats[4])
        } else {
            (stats[4], stats[3])
        };

        let mut p = Player { base_info: GameObjectBase::new(0, (-1, -1, -1), false, '@', display::WHITE, display::WHITE, true, name),
                max_hp: (15 + stat_to_mod(stats[1])) as u8, curr_hp: (15 + stat_to_mod(stats[1])) as u8,
                vision_radius: default_vision_radius, str: stats[0], con: stats[1], dex: stats[2], chr, apt, role: Role::Warrior, xp: 0, level: 1, max_depth: 0, 
                ac: 10, purse: 20, readied_weapon: "".to_string(), energy: 1.0, energy_restore: 1.0, inventory: Vec::new(), next_slot: 'a', hit_die: 10,
                stealth_score: 10, statuses: Vec::new(), size: 2,
        };
        
        // Warrior starting equipment

        if let Some(GameObjects::Item(mut spear)) = Item::get_item(game_obj_db, "spear") {
            spear.equiped = true;
            p.add_to_inv(GameObjects::Item(spear));
        }

        if let Some(GameObjects::Item(mut armour)) = Item::get_item(game_obj_db, "ringmail") {
            armour.equiped = true;

            // All this for a dumb joke...
            let r = rand::thread_rng().gen_range(0, 3);
            let s = if r == 0 {
                "Made in Middle-Earth.".to_string()
            } else if r == 1 {
                format!("Proprety of {}.", name)                
            } else {
                "Do not starch.".to_string()
            };
            armour.text = Some(("written on the label:".to_string(), s));
            p.add_to_inv(GameObjects::Item(armour));
        }

        if let Some(GameObjects::Item(dagger)) = Item::get_item(game_obj_db, "dagger") {
            p.add_to_inv(GameObjects::Item(dagger));
        }
                
        for _ in 0..5 {
            if let Some(GameObjects::Item(torch)) = Item::get_item(game_obj_db, "torch") {
                p.add_to_inv(GameObjects::Item(torch));
            }
        }

        for _ in 0..3 {
            if let Some(GameObjects::Item(scroll)) = Item::get_item(game_obj_db, "scroll of blink") {
                p.add_to_inv(GameObjects::Item(scroll));
            }
        }
        
        if let Some(GameObjects::Item(wand)) = Item::get_item(game_obj_db, "wand of frost") {
            p.add_to_inv(GameObjects::Item(wand));
        }

        p.calc_gear_effects();

        game_obj_db.add(GameObjects::Player(p));
    }

    /*
    pub fn new_rogue(game_objs: &mut XGameObjects, name: String) {
        let default_vision_radius = 99;
        let stats = roll_stats();

        let mut rng = rand::thread_rng();
        let (chr, str) = if rng.gen_range(0.0, 1.0) < 0.5 {
            (stats[3], stats[4])
        } else {
            (stats[4], stats[3])
        };

        let mut p = Player {            
            object_id: 0, name: name.clone(), max_hp: (12 + stat_to_mod(stats[2])) as u8, curr_hp: (12 + stat_to_mod(stats[2])) as u8,
                vision_radius: default_vision_radius, str, con: stats[2], dex: stats[0], chr, apt: stats[1], role: Role::Rogue, xp: 0, level: 1, max_depth: 0, ac: 10, 
                purse: 20, readied_weapon: "".to_string(), energy: 1.0, energy_restore: 1.25, inventory: Vec::new(), next_slot: 'a', hit_die: 8,
                stealth_score: 10, statuses: Vec::new(),
        };

        p.calc_gear_effects();
        
        let player_obj = XGameObject::new(0, &name, (0, 0, 0), '@', display::WHITE, display::WHITE, 
            None, None , None, None, Some(p), true);
        game_objs.add(player_obj);
    }
    */

    pub fn confused(&self) -> bool {
        for s in self.statuses.iter() {
            match s {
                Status::ConfusedUntil(_) => { return true; },
                _ => { },
            }
        }

        false
    }

    pub fn inv_slots_used(&self) -> HashSet<char> {
        let mut slots = HashSet::new();
        for i in self.inventory.iter() {
            if let GameObjects::Item(item) = i {
                slots.insert(item.slot);
            }
        }

        slots
    }

    pub fn inv_item_in_slot(&mut self, slot: char) -> Option<&mut GameObjects> {
        let mut index = - 1;
        for j in 0..self.inventory.len() {
            if let GameObjects::Item(item) = &self.inventory[j] {
                if item.slot == slot {
                    index = j as i32;
                }
            }            
        }

        if index >= 0 {
            let obj = self.inventory.get_mut(index as usize);
            obj
        } else {
            None
        }
    }

    pub fn inv_obj_of_id(&mut self, id: usize) -> Option<&mut GameObjects> {
        for j in 0..self.inventory.len() {
            if self.inventory[j].obj_id() == id {
                let obj = self.inventory.get_mut(j);
                return obj;
            }
        }
        
        None
    }

    pub fn inv_remove(&mut self, id: usize) -> Option<GameObjects> {
        for j in 0..self.inventory.len() {
            if self.inventory[j].obj_id() == id {
                let obj = self.inventory.remove(j);
                return Some(obj);
            }
        }

        None
    }

    // // Caller should check if the slot exists before calling this...
    pub fn inv_remove_from_slot(&mut self, slot: char, amt: u32) -> Result<Vec<GameObjects>, String>  {
        let mut removed = Vec::new();

        let mut count = 0;
        let mut to_remove = Vec::new();
        for j in 0..self.inventory.len() {
            if count >= amt {
                break;
            }

            if let GameObjects::Item(item) = &self.inventory[j] {
                let item_slot = item.slot;
                let equiped = item.equiped;
                let i_type = item.item_type;
                if item_slot == slot {
                    if equiped && i_type == ItemType::Armour {
                        return Err("You're wearing that!".to_string());
                    }
                    to_remove.push(item.obj_id());
                    count += 1;            
                }
            }           
        }
        
        for id in to_remove.iter() {
            let obj = self.inv_remove(*id).unwrap();
            removed.push(obj);
        }
        
        Ok(removed)
    }    

    pub fn readied_obj_ids_of_type(&self, item_type: ItemType) -> Vec<usize> {
        let mut ids = Vec::new();
        for obj in self.inventory.iter() {
            if let GameObjects::Item(item) = obj {
                if item.item_type == item_type && item.equiped {
                    ids.push(item.obj_id());
                }
            }
        }

        ids
    }
    
    pub fn inc_next_slot(&mut self) {
        let used = self.inv_slots_used();
        let mut nslot = self.next_slot;		
        loop {
            nslot = (nslot as u8 + 1) as char;
            if nslot > 'z' {
                nslot = 'a';
            }

            if !used.contains(&nslot) {
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

    pub fn add_to_inv(&mut self, mut obj: GameObjects) {
        // If the item is stackable and there's another like it, they share a slot
        let mut slot_to_use = '\0';
        if let GameObjects::Item(item) = &obj {
            if item.stackable() {
                for other in self.inventory.iter() {
                    if let GameObjects::Item(other_item) = &other {
                        if item.base_info.name == other_item.base_info.name && other_item.stackable() {
                            slot_to_use = other_item.slot;
                            break;
                        }
                    }                    
                }
            }
        }
        
        if let GameObjects::Item(item) = &mut obj {
            if slot_to_use == '\0' {
                let used = self.inv_slots_used();
                slot_to_use = item.slot;
                if slot_to_use == '\0' || used.contains(&slot_to_use) {
                    slot_to_use = self.next_slot;
                    self.inc_next_slot();
                } 
            }

            item.slot = slot_to_use;
        }
        
        self.inventory.push(obj);
    }

    pub fn inv_count_in_slot(&self, slot: char) -> usize {
        let mut count = 0;
        for obj in self.inventory.iter() {
            if let GameObjects::Item(i) = obj {
                if i.slot == slot {
                    count += 1;
                }
            }            
        }

        count
    }

    // highlight: 0 is everything, 1 is useable, 2 is equipable
    pub fn inv_menu(&self, highlight: u8) -> Vec<(String, bool)> {
        let mut items = Vec::new();
        for obj in self.inventory.iter() {
            if let GameObjects::Item(i) = obj {
                let name = i.get_fullname();
                let h = if highlight == 0 {
                    true
                } else if highlight == 1 && i.useable() {
                    true
                } else if highlight == 1 && i.item_type == ItemType::Weapon && i.equiped {
                    true
                } else if highlight == 2 && i.equipable() {
                    true
                } else {
                    false
                };
                items.push((i.slot, name, h));
            }            
        }
        
        let mut menu = Vec::new();
        let mut slots = items.iter().map(|i| i.0).collect::<Vec<char>>();
        slots.sort_unstable();
        slots.dedup();
        let mut menu_items = HashMap::new();
        for s in items {
            let counter = menu_items.entry(s.0).or_insert((s.1, 0, s.2));
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
            menu.push((s, i.2));
        }
        
        menu
    }

    pub fn ac_mods_from_gear(&self) -> (i8, u128) {
        let mut sum = 0;
        let mut attributes = 0;
        for obj in self.inventory.iter() {
            if let GameObjects::Item(item) = obj {
                if item.equiped && item.ac_bonus > 0 {
                    sum += item.ac_bonus;
                    attributes |= item.attributes;                 
                }
            }
        }
        
        (sum, attributes)
    }

    pub fn readied_weapon(&self) -> Option<(&Item, String)> {
        for j in 0..self.inventory.len() {
            if let GameObjects::Item(item) = &self.inventory[j] {
                let name = item.base_info.name.clone();
                if item.equiped && item.item_type == ItemType::Weapon {
                    return Some((&item, name))
                }
            }
        }

        None
    }

    pub fn calc_gear_effects(&mut self) {
        self.calc_ac();
        self.calc_stealth();
    }

    fn calc_ac(&mut self) {
        let mut ac: i8 = 10;        
        let (armour, attributes) = self.ac_mods_from_gear();
        
        // // Heavier armour types reduce the benefit you get from a higher dex
        let mut dex_mod = stat_to_mod(self.dex);
        if attributes & items::IA_MED_ARMOUR > 0 && dex_mod > 2 {
            dex_mod = 2;
        } else if attributes & items::IA_HEAVY_ARMOUR > 0 {
            dex_mod = 0;
        }

        ac += dex_mod + armour;

        self.ac = if ac < 0 {
            0
        } else {
            ac as u8
        };
    }

    fn calc_stealth(&mut self) {
        let mut score = 10 + stat_to_mod(self.dex);

        if self.role == Role::Rogue {
            score += 1 + self.level as i8 / 4;
        }

        // I feel like having a lit torch should also have a big
        // penalty to stealth but that might nerf Rogues too much?
        let (_, attributes) = self.ac_mods_from_gear();
        if attributes & items::IA_HEAVY_ARMOUR > 0 {
            score /= 2;
        }

        self.stealth_score = if score < 0 {
            0
        } else {
            score as u8
        };
    }

    // My idea is that the roles will have differing bonuses to attack rolls. Ie.,
    // a warrior might get an extra 1d6, a rogue an extra 1d4, wizard-types no 
    // extra dice, and they get more dice as they level up.
    pub fn attack_bonus(&mut self) -> i8 {
        let mut rng = rand::thread_rng();
        let die;
        let mut num_of_dice = 1;
        match self.role {
            Role::Warrior => {
                die = 6;
                if self.level >= 5 && self.level < 10 {
                    num_of_dice = 2;
                } else if self.level >= 10 && self.level < 15 {
                    num_of_dice = 3;                
                } else if self.level >= 5 {
                    num_of_dice = 4;
                }
            },
            Role::Rogue => {
                die = 4;
                if self.level >= 5 && self.level < 10 {
                    num_of_dice = 2;
                } else if self.level >= 10 && self.level < 15 {
                    num_of_dice = 3;                
                } else if self.level >= 5 {
                    num_of_dice = 4;
                }
            },
        }

        let roll: i8 = (0..num_of_dice).map(|_| rng.gen_range(1, die + 1)).sum();
        
        // Need to differentiate between dex and str based weapons but for now...
        roll + stat_to_mod(self.str)    
    }

    pub fn bane(&self) -> bool {
        for status in self.statuses.iter() {
            match status {
                Status::Bane(_) => return true,
                _ => { },
            }
        }

        false
    }

    pub fn blind(&self) -> bool {
        for status in self.statuses.iter() {
            match status {
                Status::BlindUntil(_) => return true,
                _ => { },
            }
        }

        false
    }

    // The player will regain HP on their own every X turns (see the game loop in main.rs)
    pub fn recover(&mut self) {
        if self.curr_hp < self.max_hp {
            self.curr_hp += 1;
        }
    }

    pub fn add_xp(&mut self, xp: u32, state: &mut GameState, loc: (i32, i32, i8)) {
        self.xp += xp;

        // If the player is less than max level, check to see if they've leveled up.
        // Also, regardless of XP gained, the player won't gain two levels at once and
        // if they somehow did, put their XP total to 1 below the next level. Ie., if 
        // a 2nd level character gets 100 xp, set them to 79, which is one below the threshold
        // for level 4.
        if self.level < 20 {
            let next_level_xp = XP_CHART[self.level as usize - 1];
            if self.xp >= next_level_xp {
                state.queued_events.push_back((EventType::LevelUp, loc, 0, None));
            }

            if self.level < 19 && self.xp >= XP_CHART[self.level as usize] {
                self.xp = XP_CHART[self.level as usize] - 1;
            }
        }
    }

    pub fn level_up(&mut self) {
        self.level += 1;
        
        // Other stuff needs to happen like more hit points, etc
        let mut rng = rand::thread_rng();
        let mut hp_roll = rng.gen_range(1, self.hit_die + 1) as i8 + stat_to_mod(self.con);
        if hp_roll < 1 {
            hp_roll = 1;
        }
        self.max_hp += hp_roll as u8;
        self.curr_hp += hp_roll as u8;
        if self.curr_hp > self.max_hp {
            self.curr_hp = self.max_hp;
        }        
    }
}

impl Person for Player {
    fn damaged(&mut self, state: &mut GameState, amount: u8, dmg_type: DamageType, _assailant_id: usize, assailant_name: &str) {
        if amount >= self.curr_hp {
            // Oh no the player has been killed :O
            self.curr_hp = 0;
            state.queued_events.push_front((EventType::PlayerKilled, (0, 0, 0), 0, Some(String::from(assailant_name))));
        } else {
            self.curr_hp -= amount;
        }

        if dmg_type == DamageType::Poison {
            state.msg_queue.push_back(Message::info("You feel ill."));
        }
    }

    fn get_hp(&self) -> (u8, u8) {
        (self.curr_hp, self.max_hp)
    }

    fn add_hp(&mut self, amt: u8) {
        self.curr_hp += amt;
    }

    fn ability_check(&self, ability: Ability) -> u8 {
        let mut rng = rand::thread_rng();
        let roll = rng.gen_range(1, 21) + 
            match ability {
                Ability::Str => stat_to_mod(self.str),
                Ability::Dex => stat_to_mod(self.dex),
                Ability::Con => stat_to_mod(self.con),
                Ability::Chr => stat_to_mod(self.chr),
                Ability::Apt => stat_to_mod(self.apt),
            };
        
        if roll < 0 {
            0
        } else {
            roll as u8
        }
    }

    // Haven't yet implemented attributes for player, but eventually they'll have fire resistance, etc,
    // from their gear and in some cases inate abilities
    fn attributes(&self) -> u128 {
        0
    }

    fn size(&self) -> u8 {
        self.size
    }

    fn mark_dead(&mut self) {
        // does nothing for the Player right now
    }

    fn alive(&self) -> bool {
        true
    }
}

impl GameObject for Player {
    fn blocks(&self) -> bool {
        true
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        self.base_info.location
    }

    fn set_loc(&mut self, loc: (i32, i32, i8)) {
        self.base_info.location = loc;
    }

    fn get_fullname(&self) -> String {
        self.base_info.name.clone()
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

impl HasStatuses for Player {
    fn get_statuses(&mut self) -> Option<&mut Vec<Status>> {
        return Some(&mut self.statuses)
    }
}

pub fn stat_to_mod(stat: u8) -> i8 {
    if stat >= 10 {
        (stat as i8 - 10) / 2
    } else {
        (stat as i8 - 11) / 2
    }
}

// Classic D&D roll 4d6 and drop lowest. (Or classic in the sense that's how 
// we did it in 2e)
fn four_d6_drop_one() -> u8 {
    let mut rng = rand::thread_rng();
    let mut rolls = vec![rng.gen_range(1, 7), rng.gen_range(1, 7), rng.gen_range(1, 7), rng.gen_range(1, 7)];
    rolls.sort_unstable();

    rolls[1..].iter().sum()
}

fn roll_stats() -> Vec<u8> {
    let mut stats = vec![four_d6_drop_one(), four_d6_drop_one(), four_d6_drop_one(), four_d6_drop_one(), four_d6_drop_one()];
    stats.sort_unstable();
    stats.reverse();

    stats
}