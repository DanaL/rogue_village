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

use rand::{Rng, prelude::SliceRandom};

use super::GameState;
use crate::game_obj::{GameObject, GameObjectDB, GameObjects, Person};
use crate::util;

pub const EF_MINOR_HEAL: u128 = 0x00000001;
pub const EF_BLINK: u128      = 0x00000002;

// Minor healing can boost the entity's HP above their max,
// but it if's already at or over max it will have no further effect
fn minor_healing<P: Person>(state: &mut GameState, user: &mut P) {
    let (curr_hp, max_hp) = user.get_hp();

    let amt = rand::thread_rng().gen_range(5, 11);
    if curr_hp < max_hp {
        user.add_hp(state, amt);
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
            if state.map[&nloc].passable() && !game_obj_db.blocking_obj_at(&nloc) {
                sqs.push(nloc);
            }
        }
    }

    let mut rng = rand::thread_rng();
    if sqs.is_empty() {
        state.write_msg_buff("The magic fizzles.");
    } else {
        let landing_spot = sqs.choose(&mut rng).unwrap();
        // I should probably call lands_on_sq() too?
        game_obj_db.set_to_loc(obj_id, *landing_spot);
    }
}

pub fn apply_effects(state: &mut GameState, obj_id: usize, game_obj_db: &mut GameObjectDB, effects: u128) {
    if effects & EF_MINOR_HEAL > 0 {
        let user = game_obj_db.get_mut(obj_id);
        if let Some(GameObjects::Player(player)) = user {
            minor_healing(state, player);
        } else if let Some(GameObjects::NPC(npc)) = user {
            minor_healing(state, npc);
        }
    }
    if effects & EF_BLINK > 0 {
        blink(state, obj_id, game_obj_db);
    }
}

