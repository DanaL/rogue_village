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

use rand::Rng;
use serde::{Serialize, Deserialize};

use super::{EventType, GameState};
use crate::actor::NPC;
use crate::player;
use crate::player::Player;
use crate::game_obj::{GameObject, GameObjects};
use crate::util::StringUtils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DamageType {
    Slashing,
    Piercing,
    Bludgeoning,
    Fire,
    Cold,
    Electricity,
    Acid,
    Poison,
}

fn monster_death_msg(monster: &GameObject, assailant_id: usize) -> String {
    if assailant_id == 0 {
        format!("You kill {}!", monster.get_npc_name(false))        
    } else {
        format!("{} dies!", monster.get_npc_name(false).capitalize())        
    }
}

// Eventually here will go checks for special damage and immunities. I'm going to need a damage type 
// enum sooner or later. How to indicate assailant_id for non-player/npc, like traps and environments?
// Maybe I'll reserve usize::MAX to indicate that sort of thing?
pub fn monster_damaged(state: &mut GameState, monster: &mut GameObject, dmg_total: u8, assailant_id: usize) {
    let curr_hp = monster.npc.as_ref().unwrap().curr_hp;

    if dmg_total >= curr_hp {
        monster.npc.as_mut().unwrap().alive = false;
        state.write_msg_buff(&monster_death_msg(monster, assailant_id));
        
    } else {
        monster.npc.as_mut().unwrap().curr_hp -= dmg_total;
    }
}

pub fn player_attacks(state: &mut GameState, player: &mut Player, opponent_id: usize, game_objs: &mut GameObjects) {
    let mut rng = rand::thread_rng();

    // Fetch the attack bonuses for the player's weapon. Do it here so that Player needs to know
    // less about GameObject and such. Am mildly regretting my great idea to treat the player's
    // inventory like it's a special location in game_objs
    let weapon_attack_bonus;
    let weapon_dmg_dice;
    let num_dmg_die;
    if let Some(weapon_info) = game_objs.readied_weapon() {
        weapon_attack_bonus = weapon_info.0.attack_bonus;
        num_dmg_die = weapon_info.0.dmg_die;
        weapon_dmg_dice = weapon_info.0.dmg_dice;
    } else {
        weapon_attack_bonus = 0;
        num_dmg_die = 1;
        weapon_dmg_dice = 2;
    }
    
    let attack_roll = rng.gen_range(1, 21) + player.attack_bonus() + weapon_attack_bonus;

    let npc = game_objs.get_mut(opponent_id).unwrap();
    if attack_roll >= npc.npc.as_ref().unwrap().ac as i8 {
        let s = format!("You hit {}!", npc.get_npc_name(false));
        state.write_msg_buff(&s);
    } else {
        let s = format!("You miss {}!", npc.get_npc_name(false));
        state.write_msg_buff(&s);
    }

    let dmg_roll: u8 = (0..num_dmg_die).map(|_| rng.gen_range(1, weapon_dmg_dice + 1)).sum();
    let dmg_total = dmg_roll as i8 + weapon_attack_bonus + player::stat_to_mod(player.str);    
    if dmg_total > 0 {
        monster_damaged(state, npc, dmg_total as u8, 0);
    }
}

pub fn player_damaged(state: &mut GameState, player: &mut Player, dmg_total: u8, assailant_id: usize) {
    if dmg_total >= player.curr_hp {
        // Oh no the player has been killed :O
        player.curr_hp = 0;
        state.queued_events.push_front((EventType::PlayerKilled, player.location, 0));
    } else {
        player.curr_hp -= dmg_total;
    }
}

pub fn monster_attacks(state: &mut GameState, monster: &mut NPC, monster_id: usize, player: &mut Player) {
    let mut rng = rand::thread_rng();

    let attack_roll = rng.gen_range(1, 21) + monster.attack_mod;

    if attack_roll >= player.ac {
        let s = format!("{} hits you!", monster.npc_name(false).capitalize());
        state.write_msg_buff(&s);        
        let dmg_roll: u8 = (0..monster.dmg_dice).map(|_| rng.gen_range(1, monster.dmg_die + 1)).sum();
        let dmg_total = (dmg_roll + monster.dmg_bonus) as i8;
        if dmg_total > 0 {
            player_damaged(state, player, dmg_total as u8, monster_id);
        }
    } else {
        let s = format!("{} misses you!", monster.npc_name(false).capitalize());
        state.write_msg_buff(&s);
    }
}