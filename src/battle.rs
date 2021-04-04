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

use super::{GameObject, GameState, Status};
use crate::display::GameUI;
use crate::effects;
use crate::npc;
use crate::player;
use crate::game_obj::{Ability, GameObjectDB, Person};
use crate::util;
use crate::util::StringUtils;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    let npc = game_obj_db.get(opponent_id).unwrap();
    let invisible_opponent = npc.hidden();

    // Fetch the attack bonuses for the player's weapon. Do it here so that Player needs to know
    // less about GameObject and such. 
    let weapon_attack_bonus;
    let weapon_dmg_dice;
    let num_dmg_die;
    let dmg_type;
    let player = game_obj_db.player().unwrap();
    let blind = player.blind();
    let baned = player.bane();
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
    let mut attack_roll = rng.gen_range(1, 21) + attack_bonus + weapon_attack_bonus;
    if blind || invisible_opponent {
        attack_roll -= 5;
    }
    if baned {
        attack_roll -= rand::thread_rng().gen_range(1, 5);
    }
    let str_mod = player::stat_to_mod(player.str);

    let mut xp_earned = 0;
    let foe = game_obj_db.npc(opponent_id).unwrap();
    if attack_roll >= foe.ac as i8 {
        let s = format!("You hit {}!", foe.npc_name(false));
        state.msg_buff.push_front(s);
        
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
        let s = if blind || invisible_opponent {
            "You swing wildly!".to_string()
        } else { 
            format!("You miss {}!", foe.npc_name(false))
        };

        state.msg_buff.push_back(s);
    }

    let sbi = state.curr_sidebar_info(game_obj_db);
    
    if xp_earned > 0 {
        let player = game_obj_db.player().unwrap();
        player.add_xp(xp_earned, state, (0, 0, 0));
    }
}

pub fn monster_attacks_player(state: &mut GameState, monster_id: usize, game_obj_db: &mut GameObjectDB) {
    let mut rng = rand::thread_rng();
    let npc = game_obj_db.npc(monster_id).unwrap();
    let monster_name_indef = npc.npc_name(true);
    let monster_name = npc.npc_name(false);
    let attack_mod = npc.attack_mod;
    let dmg_die = npc.dmg_die;
    let dmg_dice = npc.dmg_dice;
    let dmg_bonus = npc.dmg_bonus;
    let monster_dc = npc.edc;
    let monster_attributes = npc.attributes;

    let player = game_obj_db.player().unwrap();
    let mut attack_roll = rng.gen_range(1, 21) + attack_mod;
    if player.base_info.hidden {
        attack_roll -= 5;
    }    
    
    if attack_roll >= player.ac {
        let s = format!("{} hits you!", monster_name.capitalize());
        state.msg_buff.push_back(s);
        let dmg_roll: u8 = (0..dmg_dice).map(|_| rng.gen_range(1, dmg_die + 1)).sum();
        let dmg_total = (dmg_roll + dmg_bonus) as i8;
        if dmg_total > 0 {
            // I'm not yet assigning damage types to monsters so just sending Piercing as a good default
            player.damaged(state, dmg_total as u8, DamageType::Piercing, monster_id, &monster_name_indef);

            // Are there any relevant extra effects from the monster's attack?
            if monster_attributes & npc::MA_WEAK_VENOMOUS > 0 {
                let con_save = player.ability_check(Ability::Con);
                if con_save <= monster_dc {                    
                    state.msg_buff.push_back("You feel ill.".to_string());
                    effects::add_status(player, Status::WeakVenom(monster_dc));
                }
            }
        }
    } else {
        let s = format!("{} misses you!", monster_name.capitalize());
        state.msg_buff.push_back(s);
    }
}

pub fn knock_back(state: &mut GameState, game_obj_db: &mut GameObjectDB, target_loc: (i32, i32, i8)) {
    let sbi = state.curr_sidebar_info(game_obj_db);
    let p = game_obj_db.player().unwrap();
    let player_size = p.size();
    let player_loc = p.base_info.location;
    let str_check = p.ability_check(Ability::Str);

    let npc_id = game_obj_db.npc_at(&target_loc).unwrap();
    let target = game_obj_db.npc(npc_id).unwrap();
    let target_size = target.size();
    let target_str_check = target.ability_check(Ability::Str);
    let target_name = target.npc_name(false);

    if target_size > player_size {
        let s = format!("You fruitlessly hurl yourself at {}.", target_name);
        state.msg_buff.push_back(s);
    } else if str_check > target_str_check {
        let d = (target_loc.0 - player_loc.0, target_loc.1 - player_loc.1, target_loc.2 - player_loc.2);
        let new_loc = (target_loc.0 + d.0, target_loc.1 + d.1, target_loc.2 + d.2);
        
        let s = format!("You bash {}!", target_name);
        state.msg_buff.push_back(s);
        
        if !state.map[&new_loc].passable() {
            let s = format!("{} does not move.", target_name.capitalize());
            state.msg_buff.push_back(s);
        } else if let Some(bystander_id) = game_obj_db.npc_at(&new_loc) {
            let bystander = game_obj_db.npc(bystander_id).unwrap();
            let name = bystander.npc_name(false);
            let s = format!("{} blunders into {}!", target_name.capitalize(), name);
            state.msg_buff.push_back(s);
        } else {
            let s = format!("{} staggers back!", target_name.capitalize());
            state.msg_buff.push_back(s);
            super::take_step(state, game_obj_db, npc_id, target_loc, new_loc);
            state.animation_pause = true;
        }
    } else {
        let s = util::format_msg(npc_id, "hold", "[pronoun] ground!", game_obj_db);
        state.msg_buff.push_back(s);
    }
}