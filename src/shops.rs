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

use std::collections::{HashMap, HashSet};

use rand::Rng;

use super::{GameState, Status};
use crate::actor::Attitude;
use crate::dialogue::DialogueLibrary;
use crate::display::GameUI;
use crate::game_obj::GameObjects;
use crate::player::Ability;
use crate::util::StringUtils;

// Is it worth preventing the character from renting a room if it's early in the day?
// Check in isn't until 3:00pm?
fn rent_room(state: &mut GameState, game_objs: &mut GameObjects) {
    let player = game_objs.player_details();

    if player.purse < 10 {
        state.write_msg_buff("\"You can't afford a room in this establishment.\"");
        return;
    }

    player.purse -= 10;
    
    let checkout = state.turn + 2880; // renting a room is basically passing for 8 hours
    player.statuses.push(Status::RestAtInn(checkout));

    state.write_msg_buff("You check in.");
}

// Eventually, having a drink will incease the player's verve (ie., the emotional readiness for dungeoneering)
// and/or possibly get them tipsy if they indulge too much.
fn buy_drink(state: &mut GameState, game_objs: &mut GameObjects) {
    let player = game_objs.player_details();

    if player.purse == 0 {
        state.write_msg_buff("\"Hey this isn't a charity!\"");
    } else {
        player.purse -= 1;
        // more drink types eventually?
        state.write_msg_buff("You drink a refreshing ale.");
    }
}

fn buy_round(state: &mut GameState, game_objs: &mut GameObjects, patrons: &Vec<usize>) {
    let player = game_objs.player_details();

    if player.purse < patrons.len() as u32 {
        state.write_msg_buff("You can't afford to pay for everyone!");
        return;
    } 

    state.write_msg_buff("You a round for everyone in the bar.");
    player.purse -= patrons.len() as u32;

    let mut made_friend = false;
        
    for npc_id in patrons {
        let p = game_objs.player_details();
        let persuasion = p.ability_check(Ability::Chr);

        if persuasion >= 13 {
            let patron = game_objs.get_mut(*npc_id).unwrap();
            patron.npc.as_mut().unwrap().attitude = Attitude::Friendly;
            made_friend = true;
        }
    }

    if made_friend {
        state.write_msg_buff("Cheers!!");
    }
}

fn inn_patrons(state: &GameState, game_objs: &mut GameObjects, innkeeper_id: usize) -> Vec<usize> {
    let mut patrons = Vec::new();
    if let Some(buildings) = &state.world_info.town_buildings {
        for sq in buildings.tavern.iter() {
            if let Some(npc_id) = game_objs.npc_at(&sq) {
                patrons.push(npc_id);
            }
        }
    }

    patrons.retain(|p| *p != innkeeper_id);

    patrons
}

pub fn talk_to_innkeeper(state: &mut GameState, innkeeper_id: usize, game_objs: &mut GameObjects, 
        dialogue: &DialogueLibrary, gui: &mut GameUI) {
    let patrons = inn_patrons(state, game_objs, innkeeper_id);
    let npc = game_objs.get_mut(innkeeper_id).unwrap();
    let mut msg = npc.npc.as_mut().unwrap().talk_to(state, dialogue, npc.location, None);
    state.add_to_msg_history(&msg);
    
    msg.push('\n');
    msg.push('\n');
    msg.push_str("a) buy a drink (1$)\n");
    msg.push_str("b) rent a room (10$)\n");
    if !patrons.is_empty() {
        let s = format!("c) buy a round for the bar ({}$)", patrons.len());
        msg.push_str(&s);
    }

    let name = format!("{}, the innkeeper", npc.get_npc_name(true).capitalize());
    let options: HashSet<char> = vec!['a', 'b', 'c'].into_iter().collect();

    let answer = gui.popup_menu(&name, &msg, options);
    if let Some(ch) = answer {
        if ch == 'a' {
            buy_drink(state, game_objs);
        } else if ch == 'b' {
            rent_room(state, game_objs);
        } else if ch == 'c' {
            buy_round(state, game_objs, &patrons);
        }
    } else {
        let x = rand::thread_rng().gen_range(0, 3);
        if x == 0 {
            state.write_msg_buff("\"Nevermind.\"");
        } else if x == 1 {
            state.write_msg_buff("\"No loitering.\"");
        } else {
            state.write_msg_buff("\"No outside food or drink.\"");
        }
    }
}

pub fn talk_to_grocer(state: &mut GameState, grocer_id: usize, game_objs: &mut GameObjects,
        dialogue: &DialogueLibrary, gui: &mut GameUI) {
    let grocer = game_objs.get_mut(grocer_id).unwrap();

    let mut extra_info = HashMap::new();
    extra_info.insert("#goods#".to_string(), "adventuring supplies".to_string());
    let mut msg = grocer.npc.as_mut().unwrap().talk_to(state, dialogue, grocer.location, Some(extra_info));
    state.add_to_msg_history(&msg);

    let name = format!("{}, the grocer", grocer.get_npc_name(true).capitalize());
    gui.popup_msg(&name, &msg);
}