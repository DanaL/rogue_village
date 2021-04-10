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
use crate::util;

pub const EF_MINOR_HEAL: u128     = 0x00000001;
pub const EF_BLINK: u128          = 0x00000002;
pub const EF_WEAK_VENOM: u128     = 0x00000004;
pub const EF_WEAK_BLINDNESS: u128 = 0x00000008;

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
fn minor_healing(state: &mut GameState, user: &mut dyn Person) {
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
            minor_healing(state, user);
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
}

// Constants used to track abilities that have cool down times
pub const AB_CREATE_PHANTASM: u16 = 0;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Status {
    PassUntil(u32),
    RestAtInn(u32),
    WeakVenom(u8),
    BlindUntil(u32),
    Bane(u32),
    Invisible(u32),
    FadeAfter(u32), // used for illusions that will disappear after a certain time or when their creator dies
    CoolingDown(u16, u32),
}

pub trait HasStatuses {
    fn get_statuses(&mut self) -> Option<&mut Vec<Status>>;
}

pub fn add_status<T: HasStatuses + GameObject>(person: &mut T, status: Status) {
    if let Status::Invisible(_) = status  {
        person.hide();
    }

    let statuses = person.get_statuses().unwrap();

    // BlindUtil and other statuses we want to merge. Ie., if someone has BlindUntil(100) and then
    // gets further blinded, replace the current status with the one that's further in the future.
    if let Status::BlindUntil(new_time) = status {
        for j in 0..statuses.len() {
            if let Status::BlindUntil(prev_time) = statuses[j] {
                if new_time > prev_time {
                    statuses[j] = status;
                    return;
                }
            }
        }
    }

    if let Status::Bane(new_time) = status {
        for j in 0..statuses.len() {
            if let Status::Bane(prev_time) = statuses[j] {
                if new_time > prev_time {
                    statuses[j] = status;
                    return;
                }
            }
        }
    }

    if let Status::Invisible(new_time) = status {
        for j in 0..statuses.len() {
            if let Status::Invisible(prev_time) = statuses[j] {
                if new_time > prev_time {
                    statuses[j] = status;
                    return;
                }
            }
        }
    }

    if let Status::CoolingDown(ability, time) = status {
        for j in 0..statuses.len() {
            if let Status::CoolingDown(curr_ability, curr_time) = statuses[j] {
                if ability == curr_ability && time > curr_time {
                    statuses[j] = status;
                    return;
                }
            }
        }
    }

    // Generally don't allow the player to have more than one status effect of the same type.
    for s in statuses.iter() {
        if *s == status {
            return;
        }
    }

    statuses.push(status);
}

pub fn remove_status<T: HasStatuses + GameObject>(person: &mut T, status: Status) {
    let statuses = person.get_statuses().unwrap();
    statuses.retain(|s| *s != status);

    if let Status::Invisible(_) = status {
        person.reveal();
    }
}

pub fn check_statuses<T: HasStatuses + GameObject + Person>(person: &mut T, state: &mut GameState) -> Option<Vec<String>> {
    let obj_id = person.obj_id();
    let con_check = person.ability_check(Ability::Con); // gotta do this here for borrow checker reasons...
    let statuses = person.get_statuses().unwrap();
    let mut messages = Vec::new();

    let mut reveal = false;
    let mut killed = false;
    let mut j = 0;
    while j < statuses.len() {
        match statuses[j] {
            Status::PassUntil(time) => {
                if time <= state.turn {
                    statuses.remove(j);
                    continue;
                }
            },
            Status::BlindUntil(time) => {
                if time <= state.turn {
                    statuses.remove(j);
                    if obj_id == 0 {
                        messages.push("Your vision clears!".to_string());
                    }
                    continue;
                }
            },
            Status::Bane(time) => {
                if time <= state.turn {
                    statuses.remove(j);
                    if obj_id == 0 {
                        messages.push("A curse lifts!".to_string());
                    }
                    continue;
                }
            },
            Status::RestAtInn(time) => {
                if time <= state.turn {
                    statuses.remove(j);
                    if obj_id == 0 {
                        messages.push("You awake feeling refreshed!".to_string());
                    }
                    continue;
                }
            },
            Status::WeakVenom(dc) => {
                if con_check >= dc {
                    if obj_id == 0 {
                        messages.push("You feel better".to_string());
                    }
                    statuses.remove(j);                    
                }
            },
            Status::Invisible(time) => {
                if time <= state.turn {
                    statuses.remove(j);
                    reveal = true;
                }
            },            
            Status::FadeAfter(time) => {
                if time <= state.turn {
                    killed = true;
                }
            }
            Status::CoolingDown(_, time) => {
                if time <= state.turn {
                    statuses.remove(j);
                }
            }
        }

        j += 1;
    }

    if reveal {
        person.reveal();
        if obj_id == 0 {
            messages.push("You reappear!".to_string());
        } else {
            let s = format!("The {} re-appears!", person.get_fullname());
            messages.push(s.to_string());
        }
    }

    if killed {
        let name = person.get_fullname();
        person.mark_dead();
        let s = format!("The {} evaporates into mist!", name);
        messages.push(s.to_string());
    }

    if messages.is_empty() {
        None
    } else {
        Some(messages)
    }
}
