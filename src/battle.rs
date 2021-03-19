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

use super::GameState;
use crate::actor::NPC;
use crate::player;
use crate::game_obj::{GameObjectDB, Person};
use crate::util::StringUtils;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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

pub fn player_attacks(state: &mut GameState, opponent_id: usize, game_obj_db: &mut GameObjectDB) {
    let mut rng = rand::thread_rng();

    // Fetch the attack bonuses for the player's weapon. Do it here so that Player needs to know
    // less about GameObject and such. 
    let weapon_attack_bonus;
    let weapon_dmg_dice;
    let num_dmg_die;
    let dmg_type;
    let player = game_obj_db.player().unwrap();
    if let Some(weapon_info) = player.readied_weapon() {
        weapon_attack_bonus = weapon_info.0.attack_bonus;
        dmg_type = weapon_info.0.dmg_type;
        num_dmg_die = weapon_info.0.dmg_die;
        weapon_dmg_dice = weapon_info.0.dmg_dice;
    } else {
        weapon_attack_bonus = 0;
        num_dmg_die = 1;
        weapon_dmg_dice = 2;
        dmg_type = DamageType::Bludgeoning; 
    }
    
    let attack_bonus = player.attack_bonus();
    let attack_roll = rng.gen_range(1, 21) + attack_bonus + weapon_attack_bonus;
    let str_mod = player::stat_to_mod(player.str);

    let mut xp_earned = 0;
    let foe = game_obj_db.npc(opponent_id).unwrap();
    if attack_roll >= foe.ac as i8 {
        let s = format!("You hit {}!", foe.npc_name(false));
        state.write_msg_buff(&s);

        let dmg_roll: u8 = (0..num_dmg_die).map(|_| rng.gen_range(1, weapon_dmg_dice + 1)).sum();
        let dmg_total = dmg_roll as i8 + weapon_attack_bonus + str_mod;    
        if dmg_total > 0 {
            //let monster = npc.npc.as_mut().unwrap();
            foe.damaged(state, dmg_total as u8, dmg_type, 0, "player");

            // I don't know if this is the best spot for this? But for now, if the monsters is no longer
            // alive after the player must have killed it so award xp
            if !foe.alive {
                xp_earned = foe.xp_value;
            }
        }
    } else {
        let s = format!("You miss {}!", foe.npc_name(false));
        state.write_msg_buff(&s);
    }

    if xp_earned > 0 {
        let player = game_obj_db.player().unwrap();
        player.add_xp(xp_earned, state, (0, 0, 0));
    }
}

pub fn monster_attacks_player(state: &mut GameState, monster: &mut NPC, monster_id: usize, game_obj_db: &mut GameObjectDB) {
    let mut rng = rand::thread_rng();

    let attack_roll = rng.gen_range(1, 21) + monster.attack_mod;

    let player = game_obj_db.player().unwrap();
    if attack_roll >= player.ac {
        let s = format!("{} hits you!", monster.npc_name(false).capitalize());
        state.write_msg_buff(&s);        
        let dmg_roll: u8 = (0..monster.dmg_dice).map(|_| rng.gen_range(1, monster.dmg_die + 1)).sum();
        let dmg_total = (dmg_roll + monster.dmg_bonus) as i8;
        if dmg_total > 0 {
            // I'm not yet assigning damage types to monsters so just sending Piercing as a good default
            player.damaged(state, dmg_total as u8, DamageType::Piercing, monster_id, &monster.npc_name(true));
        }
    } else {
        let s = format!("{} misses you!", monster.npc_name(false).capitalize());
        state.write_msg_buff(&s);
    }
}