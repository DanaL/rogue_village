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

use super::GameState;
use crate::display;
use crate::{EventType, items};
use crate::game_obj::{GameObject, GameObjects};
use crate::items::{Item, ItemType};
use crate::util::StringUtils;

const XP_CHART: [u32; 19] = [20, 40, 80, 160, 320, 640, 1280, 2560, 5210, 10_000, 15_000, 21_000, 28_000, 36_000, 44_000, 52_000, 60_000, 68_000, 76_000];

pub enum Ability {
    Str,
    Dex,
    Con,
    Chr,
    Apt,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub object_id: usize,
	pub name: String,
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
    pub inventory: Vec<GameObject>,
    pub next_slot: char,
    pub hit_die: u8,
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

        // Announce sunrise and sunset if the player is on the surface
        // This should be here and is a dumb calculation because vision radius will be
        // affected by say torches. It should be moved to end-of-turn stuff in the gameloop
        if prev_vr == 99 && self.vision_radius == 9 && loc.2 == 0 {
            state.write_msg_buff("The sun is beginning to set.");
        }
        if prev_vr == 5 && self.vision_radius == 7 && loc.2 == 0 {
            state.write_msg_buff("Sunrise soon.");
        }
    }

    pub fn new_warrior(game_objs: &mut GameObjects, name: String) {
        let default_vision_radius = 99;
        let stats = roll_stats();
        
        let mut rng = rand::thread_rng();
        let (chr, apt) = if rng.gen_range(0.0, 1.0) < 0.5 {
            (stats[3], stats[4])
        } else {
            (stats[4], stats[3])
        };

        let mut p = Player {            
            object_id: 0, name: name.clone(), max_hp: (15 + stat_to_mod(stats[1])) as u8, curr_hp: (15 + stat_to_mod(stats[1])) as u8,
                vision_radius: default_vision_radius, str: stats[0], con: stats[1], dex: stats[2], chr, apt, role: Role::Warrior, xp: 0, level: 1, max_depth: 0, 
                ac: 10, purse: 20, readied_weapon: "".to_string(), energy: 1.0, energy_restore: 1.0, inventory: Vec::new(), next_slot: 'a', hit_die: 10,
        };
        
        // Warrior starting equipment
        let mut sword = Item::get_item(game_objs, "longsword").unwrap();        
        sword.item.as_mut().unwrap().equiped = true;
        p.add_to_inv(sword);
                
        let mut armour = Item::get_item(game_objs, "ringmail").unwrap();
        armour.item.as_mut().unwrap().equiped = true;
        p.add_to_inv(armour);
        
        let dagger = Item::get_item(game_objs, "dagger").unwrap();
        p.add_to_inv(dagger);
        
        for _ in 0..5 {
            let t = Item::get_item(game_objs, "torch").unwrap();
            p.add_to_inv(t);
        }
        
        p.calc_ac();

        let player_obj = GameObject::new(0, &name, (0, 0, 0), '@', display::WHITE, display::WHITE, 
            None, None , None, None, Some(p), true);
        game_objs.add(player_obj);
    }

    pub fn new_rogue(game_objs: &mut GameObjects, name: String) {
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
        };

        p.calc_ac();
        
        let player_obj = GameObject::new(0, &name, (0, 0, 0), '@', display::WHITE, display::WHITE, 
            None, None , None, None, Some(p), true);
        game_objs.add(player_obj);
    }

    pub fn inv_slots_used(&self) -> HashSet<char> {
        self.inventory.iter()
            .map(|i| i.item.as_ref().unwrap().slot)
            .collect::<HashSet<char>>()
    }

    pub fn inv_item_in_slot(&mut self, slot: char) -> Option<&mut GameObject> {
        for j in 0..self.inventory.len() {
            if self.inventory[j].item.as_ref().unwrap().slot == slot {
                let obj = self.inventory.get_mut(j);
                return obj;
            }
        }

        None
    }

    pub fn inv_obj_of_id(&mut self, id: usize) -> Option<&mut GameObject> {
        for j in 0..self.inventory.len() {
            if self.inventory[j].object_id == id {
                let obj = self.inventory.get_mut(j);
                return obj;
            }
        }
        
        None
    }

    pub fn inv_remove(&mut self, id: usize) -> Option<GameObject> {
        for j in 0..self.inventory.len() {
            if self.inventory[j].object_id == id {
                let obj = self.inventory.remove(j);
                return Some(obj);
            }
        }

        None
    }

    // // Caller should check if the slot exists before calling this...
    pub fn inv_remove_from_slot(&mut self, slot: char, amt: u32) -> Result<Vec<GameObject>, String>  {
        let mut removed = Vec::new();

        let mut count = 0;
        for j in 0..self.inventory.len() {
            if count >= amt {
                break;
            }

            let obj_id = self.inventory[j].object_id;
            let details = self.inventory[j].item.as_ref().unwrap();
            let item_slot = details.slot;
            let equiped = details.equiped;
            let i_type = details.item_type;
            if item_slot == slot {
                if equiped && i_type == ItemType::Armour {
                    return Err("You're wearing that!".to_string());
                }
                
                let obj = self.inv_remove(obj_id).unwrap();
                removed.push(obj);
                count += 1;            
            }
            
        }     

        Ok(removed)
    }    

    pub fn readied_obj_ids_of_type(&self, item_type: ItemType) -> Vec<usize> {
        let mut ids = Vec::new();
        for obj in self.inventory.iter() {
            let item = obj.item.as_ref().unwrap();
            if item.item_type == item_type && item.equiped {
                ids.push(obj.object_id);
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

    pub fn add_to_inv(&mut self, mut obj: GameObject) {
        // If the item is stackable and there's another like it, they share a slot
        if obj.item.as_ref().unwrap().stackable() {
            for other in self.inventory.iter() {
                let other_item = other.item.as_ref().unwrap();
                if obj.name == other.name && other_item.stackable {
                    obj.item.as_mut().unwrap().slot = other_item.slot;
                    self.inventory.push(obj);
                    return;
                }
            }
        }

        let used = self.inv_slots_used();
        let curr_slot = obj.item.as_ref().unwrap().slot;
        if curr_slot == '\0' || used.contains(&curr_slot) {
            obj.item.as_mut().unwrap().slot = self.next_slot;
            self.inc_next_slot();
        }
        self.inventory.push(obj);
    }

    pub fn inv_count_in_slot(&self, slot: char) -> usize {
        let mut count = 0;
        for obj in self.inventory.iter() {
            if obj.item.as_ref().unwrap().slot == slot {
                count += 1;
            }
        }

        count
    }

    pub fn inv_menu(&self) -> Vec<String> {
        let mut items = Vec::new();
        for obj in self.inventory.iter() {
            let name = obj.get_fullname();
            items.push((obj.item.as_ref().unwrap().slot, name));
        }
        
        let mut menu = Vec::new();
        let mut slots = items.iter().map(|i| i.0).collect::<Vec<char>>();
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
        for obj in self.inventory.iter() {
            let item = obj.item.as_ref().unwrap();
            if item.equiped && item.ac_bonus > 0 {
                sum += item.ac_bonus;
                attributes |= item.attributes;                 
            }
        }
        
        (sum, attributes)
    }

    pub fn readied_weapon(&self) -> Option<(&Item, String)> {
        for j in 0..self.inventory.len() {
            let name = self.inventory[j].get_fullname();
            let item = self.inventory[j].item.as_ref().unwrap();
            if item.equiped && item.item_type == ItemType::Weapon {
                return Some((item, name))
            }
        }

        None
    }

    pub fn calc_ac(&mut self) {
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

    pub fn ability_check(&self, ability: Ability) -> u8 {
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

    pub fn add_hp(&mut self, amt: u8) {
        self.curr_hp += amt;

        if self.curr_hp > self.max_hp {
            self.curr_hp = self.max_hp;
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

    pub fn level_up(&mut self, state: &mut GameState) {
        self.level += 1;
        let s = format!("Welcome to level {}!", self.level);

        println!("?? {}", self.max_hp);
        // Other stuff needs to happen like more hit points, etc
        let mut rng = rand::thread_rng();
        let mut hp_roll = rng.gen_range(1, self.hit_die + 1) as i8 + stat_to_mod(self.con);
        if hp_roll < 1 {
            hp_roll = 1;
        }
        self.max_hp += hp_roll as u8;
        self.add_hp(hp_roll as u8);
        println!("?? {}", self.max_hp);
        state.write_msg_buff(&s);
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