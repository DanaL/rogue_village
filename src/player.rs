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

use rand::Rng;

use super::GameState;
use crate::items;
use crate::items::{Inventory, Item};

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct Player {
	pub name: String,
	pub max_hp: i8,
	pub curr_hp: i8,
	pub location: (i32, i32, i8),
    pub vision_radius: u8,
    pub str: u8,
    pub dex: u8,
    pub con: u8,
    pub chr: u8,
    pub apt: u8,
    pub role: Role,
    xp: u32,
    pub level: u8,
    pub max_depth: u8,
    pub inventory: Inventory,
    pub ac: u8,
}

impl Player {
    pub fn calc_vision_radius(&mut self, state: &mut GameState) {
        let prev_vr = self.vision_radius;
        let (hour, _) = state.curr_time();

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

        // Announce sunrise and sunset if the player is on the surface
        if prev_vr == 99 && self.vision_radius == 9 && self.location.2 == 0 {
            state.write_msg_buff("The sun is beginning to set.");
        }
        if prev_vr == 5 && self.vision_radius == 7 && self.location.2 == 0 {
            state.write_msg_buff("Sunrise soon.");
        }
    }

    pub fn new_warrior(name: String) -> Player {
        let default_vision_radius = 99;
        let stats = roll_stats();
        
        let mut rng = rand::thread_rng();
        let (chr, apt) = if rng.gen_range(0.0, 1.0) < 0.5 {
            (stats[3], stats[4])
        } else {
            (stats[4], stats[3])
        };

        let mut p = Player {            
            name, max_hp: 15 + stat_to_mod(stats[1]), curr_hp: 15 + stat_to_mod(stats[1]), location: (0, 0, 0), vision_radius: default_vision_radius,
                str: stats[0], con: stats[1], dex: stats[2], chr, apt, role: Role::Warrior, xp: 0, level: 1, max_depth: 0, inventory: Inventory::new(),
                ac: 10,
        };

        // Warrior starting equipment
        let mut sword = Item::get_item("longsword").unwrap();
        sword.equiped = true; 
        let mut armour = Item::get_item("ringmail").unwrap();
        armour.equiped = true;
        
        p.inventory.add(sword);
        p.inventory.add(armour);
        p.inventory.purse = 20;

        p.calc_ac();

        p
    }

    pub fn new_rogue(name: String) -> Player {
        let default_vision_radius = 99;
        let stats = roll_stats();

        let mut rng = rand::thread_rng();
        let (chr, str) = if rng.gen_range(0.0, 1.0) < 0.5 {
            (stats[3], stats[4])
        } else {
            (stats[4], stats[3])
        };

        let mut p = Player {            
            name, max_hp: 12 + stat_to_mod(stats[2]), curr_hp: 12 + stat_to_mod(stats[2]), location: (0, 0, 0), vision_radius: default_vision_radius,
                str, con: stats[2], dex: stats[0], chr, apt: stats[1], role: Role::Rogue, xp: 0, level: 1, max_depth: 0, inventory: Inventory::new(),
                ac: 10,
        };

        p.calc_ac();

        p
    }

    pub fn calc_ac(&mut self) {
        let mut ac: i8 = 10;
        let mut attributes = 0;
        let slots = self.inventory.used_slots();        
        for s in slots {
            let i = self.inventory.peek_at(s).unwrap();
            // at some point there might be items that give you a bonus or penalty
            // even if they aren't equiped? I guess I'd handle that with an attribute maybe?
            if i.equiped {
                ac += i.ac_bonus;
                attributes |= i.attributes;
            }
        }

        // Heavier armour types reduce the benefit you get from a higher dex
        let mut dex_mod = stat_to_mod(self.dex);
        if attributes & items::IA_MED_ARMOUR > 0 && dex_mod > 2 {
            dex_mod = 2;
        } else if attributes & items::IA_HEAVY_ARMOUR > 0 {
            dex_mod = 0;
        }

        ac += dex_mod;

        self.ac = if ac < 0 {
            0
        } else {
            ac as u8
        };
    }
}

fn stat_to_mod(stat: u8) -> i8 {
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
    rolls.sort();

    rolls[1..].iter().sum()
}

fn roll_stats() -> Vec<u8> {
    let mut stats = vec![four_d6_drop_one(), four_d6_drop_one(), four_d6_drop_one(), four_d6_drop_one(), four_d6_drop_one()];
    stats.sort();
    stats.reverse();

    stats
}