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

extern crate rand;
extern crate serde;

use std::u128;

use rand::{Rng, prelude::SliceRandom};
use serde::{Serialize, Deserialize};

use super::{GameState, Message};
use crate::battle::DamageType;
use crate::game_obj::{Ability, GameObject, GameObjectDB, Person};
use crate::map::Tile;
use crate::util;

pub const EF_MINOR_HEAL: u128     = 0x00000001;
pub const EF_BLINK: u128          = 0x00000002;
pub const EF_WEAK_VENOM: u128     = 0x00000004;
pub const EF_WEAK_BLINDNESS: u128 = 0x00000008;
pub const EF_FROST: u128          = 0x00000010;
pub const EF_LEVITATION: u128     = 0x00000020;

fn apply_xp(state: &mut GameState, game_obj_db: &mut GameObjectDB, xp: u32) {
    let player = game_obj_db.player().unwrap();
    player.add_xp(xp, state, (0, 0, 0));
}

pub fn frost(state: &mut GameState, game_obj_db: &mut GameObjectDB, loc: (i32, i32, i8), src_obj_id: usize) {
    if state.map[&loc] == Tile::Water || state.map[&loc] == Tile::DeepWater || state.map[&loc] == Tile::UndergroundRiver {
        state.map.insert(loc, Tile::Ice);
        state.msg_queue.push_back(Message::new(0, loc, "The water freezes over!", "You hear a cracking sound."));
        // need to add in an event for the ice to later melt
    }

    let mut killed_by_effect = None; 
    if let Some(victim_id) = game_obj_db.person_at(loc) {
        let dmg = rand::thread_rng().gen_range(1, 9) + rand::thread_rng().gen_range(1, 9) + rand::thread_rng().gen_range(1, 9);
        let victim = game_obj_db.as_person(victim_id).unwrap();
        victim.damaged(state, dmg, DamageType::Cold, 0, "frost");

        if !victim.alive() {
            killed_by_effect = Some(victim_id)
        } 
    }

    if let Some(id) = killed_by_effect {
        let xp = if let Some(npc) = game_obj_db.npc(id) {
            npc.xp_value
        } else {
            0
        };

        if xp > 0 {
            apply_xp(state, game_obj_db, xp);
        }
    }
}

// Short range, untargeted teleport
fn blink(state: &mut GameState, obj_id: usize, game_obj_db: &mut GameObjectDB) {
    let obj = game_obj_db.get_mut(obj_id).unwrap();
    let loc = obj.get_loc();

    let mut sqs = Vec::new();
    for radius in 5..11 {
        let circle = util::bresenham_circle(loc.0, loc.1, radius);
        for pt in circle {
            let nloc = (pt.0, pt.1, loc.2);
            if state.map.contains_key(&nloc) && state.map[&nloc].passable() && !game_obj_db.blocking_obj_at(&nloc) {
                sqs.push(nloc);
            }
        }
    }

    let mut rng = rand::thread_rng();
    if sqs.is_empty() {
        state.msg_queue.push_back(Message::new(obj_id, loc, "The magic fizzles", ""));
    } else {
        let landing_spot = sqs.choose(&mut rng).unwrap();
        // I should probably call lands_on_sq() too?
        game_obj_db.set_to_loc(obj_id, *landing_spot);        
    }
}

// Minor healing can boost the entity's HP above their max,
// but it if's already at or over max it will have no further effect
fn minor_healing(user: &mut dyn Person) {
    let (curr_hp, max_hp) = user.get_hp();

    let amt = rand::thread_rng().gen_range(5, 11);
    if curr_hp < max_hp {
        user.add_hp(amt);
    } 
}

fn weak_venom(state: &mut GameState, victim: &mut dyn Person) {
    let dmg = rand::thread_rng().gen_range(1, 5);
    victim.damaged(state, dmg, DamageType::Poison, 0, "poison");
}

pub fn apply_effects(state: &mut GameState, obj_id: usize, game_obj_db: &mut GameObjectDB, effects: u128) {
    if effects & EF_MINOR_HEAL > 0 {
        if let Some(user) = game_obj_db.as_person(obj_id) {
            minor_healing(user);
        }        
    }

    if effects & EF_BLINK > 0 {
        blink(state, obj_id, game_obj_db);
    }

    if effects & EF_WEAK_VENOM > 0 {
        if let Some(victim) = game_obj_db.as_person(obj_id) {
            weak_venom(state, victim);
        }
    }

    if effects & EF_LEVITATION > 0 {
        if obj_id == 0 {
            let player = game_obj_db.player().unwrap();
            add_status(player, Status::Flying, state.turn + 50);
            state.msg_queue.push_back(Message::info("You begin to float."));
        } else {
            let npc = game_obj_db.npc(obj_id).unwrap();
            add_status(npc, Status::Flying, state.turn + 50);
        }        
    }
}

// Constants used to track abilities that have cool down times
pub const AB_CREATE_PHANTASM: u128 = 0;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Passing,
    RestAtInn,
    WeakVenom,
    Blind,
    Bane,
    Invisible,
    FadeAfter, // used for illusions that will disappear after a certain time or when their creator dies
    CoolingDown(u128),
    Confused,
    Paralyzed,
    Flying,
}

pub trait HasStatuses {
    fn get_statuses(&mut self) -> Option<&mut Vec<(Status, u32)>>;
}

pub fn add_status<T: HasStatuses + GameObject>(person: &mut T, status: Status, time: u32) {
    if status == Status::Invisible {
        person.hide();
    }

    let statuses = person.get_statuses().unwrap();

    // BlindUtil and other statuses we want to merge. Ie., if someone has BlindUntil(100) and then
    // gets further blinded, replace the current status with the one that's further in the future.
    for j in 0..statuses.len() {
        if status == Status::Blind && statuses[j].0 == Status::Blind && time > statuses[j].1 {
            statuses[j].1 = time;
            return;
        }
        if status == Status::Bane && statuses[j].0 == Status::Bane && time > statuses[j].1 {
            statuses[j].1 = time;
            return;
        }
        if status == Status::Invisible && statuses[j].0 == Status::Invisible && time > statuses[j].1 {
            statuses[j].1 = time;
            return;
        }
        if status == Status::Confused && statuses[j].0 == Status::Confused && time > statuses[j].1 {
            statuses[j].1 = time;
            return;
        }
        if status == Status::Flying && statuses[j].0 == Status::Flying && time > statuses[j].1 {
            statuses[j].1 = time;
            return;
        }
    }
    
    if let Status::CoolingDown(ability) = status {
        for j in 0..statuses.len() {
            if let Status::CoolingDown(curr_ability) = statuses[j].0 {
                if ability == curr_ability && time > statuses[j].1 {
                    statuses[j].1 = time;
                    return;
                }
            }
        }
    }

    // Generally don't allow the player to have more than one status effect of the same type.
    for s in statuses.iter() {
        if s.0 == status {
            return;
        }
    }
    
    statuses.push((status, time));
}

pub fn remove_status<T: HasStatuses + GameObject>(person: &mut T, status: Status) {
    let statuses = person.get_statuses().unwrap();
    statuses.retain(|s| s.0 != status);

    if status == Status::Invisible {
        person.reveal();
    }
}

pub fn check_statuses<T: HasStatuses + GameObject + Person>(person: &mut T, state: &mut GameState) {
    let obj_id = person.obj_id();
    let con_check = person.ability_check(Ability::Con) as u32; // gotta do this here for borrow checker reasons...
    let statuses = person.get_statuses().unwrap();
    
    let mut reveal = false;
    let mut killed = false;
    for j in 0..statuses.len() {
        if statuses[j].0 == Status::Passing && statuses[j].1 <= state.turn {
            statuses.remove(j);
            continue;
        }
        if statuses[j].0 == Status::Blind && statuses[j].1 <= state.turn {
            statuses.remove(j);
            if obj_id == 0 {
                state.msg_queue.push_back(Message::info("Your vision clears!"));
            }
            continue;
        }
        if statuses[j].0 == Status::Bane && statuses[j].1 <= state.turn {
            statuses.remove(j);
            if obj_id == 0 {
                state.msg_queue.push_back(Message::info("A curse lifts!"));
            }
            continue;
        }
        if statuses[j].0 == Status::RestAtInn && statuses[j].1 <= state.turn {
            statuses.remove(j);
            if obj_id == 0 {
                state.msg_queue.push_back(Message::info("You awake feeling refreshed."));
            }
            continue;
        }
        if statuses[j].0 == Status::WeakVenom && con_check > statuses[j].1 {
            statuses.remove(j);
            if obj_id == 0 {
                state.msg_queue.push_back(Message::info("You feel better!"));
            }
            continue;
        }
        if statuses[j].0 == Status::Paralyzed && con_check > statuses[j].1 {
            statuses.remove(j);
            if obj_id == 0 {
                state.msg_queue.push_back(Message::info("You muscles work again!"));
            }
            continue;
        }
        if statuses[j].0 == Status::Invisible && statuses[j].1 <= state.turn {
            statuses.remove(j);
            reveal = true;
            continue;
        }
        if statuses[j].0 == Status::FadeAfter && statuses[j].1 <= state.turn {
            statuses.remove(j);
            killed = true;
            continue;
        }
        if let Status::CoolingDown(_) = statuses[j].0 {
            if statuses[j].1 <= state.turn {                
                statuses.remove(j);
                continue;
            }
        }
        if statuses[j].0 == Status::Confused && statuses[j].1 <= state.turn {
            statuses.remove(j);
            if obj_id == 0 {
                state.msg_queue.push_back(Message::info("You shake off your confusion."));
            }
            continue;
        }
        if statuses[j].0 == Status::Flying {
            if statuses[j].1 <= state.turn {
                statuses.remove(j);
                if obj_id == 0 {
                    state.msg_queue.push_back(Message::info("Your flight ends."));
                }
                continue;
            } else if statuses[j].1 - state.turn == 10 && obj_id == 0 {
                state.msg_queue.push_back(Message::info("You wobble in the air."));
                continue;
            }
            // Hmm should trigger the effect of landing on a square here but that may be tricky :/
        }
    }

    if reveal {
        person.reveal();
        if obj_id == 0 {
            state.msg_queue.push_back(Message::info("You reappear!"));            
        } else {
            let s = format!("The {} re-appears!", person.get_fullname());
            state.msg_queue.push_back(Message::new(obj_id, person.get_loc(), &s, ""));    
        }
    }

    if killed {
        let name = person.get_fullname();
        person.mark_dead();
        let s = format!("The {} evaporates into mist!", name);
        state.msg_queue.push_back(Message::new(obj_id, person.get_loc(), &s, ""));         
    }    
}
