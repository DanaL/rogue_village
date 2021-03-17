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

use rand::Rng;

use super::GameState;
use crate::game_obj::Person;

pub const EF_MINOR_HEAL: u128 = 0x00000001;

// Minor healing can boost the entity's HP above their max,
// but it if's already at or over max it will have no further effect
fn minor_healing(state: &mut GameState, user: &mut dyn Person) {
    let (curr_hp, max_hp) = user.get_hp();

    let amt = rand::thread_rng().gen_range(5, 11);
    if curr_hp < max_hp {
        user.add_hp(state, amt);
    } 
}

pub fn apply_effects(state: &mut GameState, user: &mut dyn Person, effects: u128) {
    if effects & EF_MINOR_HEAL > 0 {
        minor_healing(state, user);
    }
}

