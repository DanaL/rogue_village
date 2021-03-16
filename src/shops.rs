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

use std::collections::{HashMap, HashSet, VecDeque};

use rand::Rng;

use super::{GameState, Status};
use crate::actor::Attitude;
use crate::dialogue::DialogueLibrary;
use crate::display::GameUI;
use crate::game_obj::{GameObject, GameObjects};
use crate::items::Item;
use crate::player::Ability;
use crate::util::StringUtils;

// This is similar to but not quite the same as the player inventory tool
fn inventory_menu(inventory: &Vec<GameObject>) -> Vec<(String, char, u8, u16)> {    
    let mut items: Vec<(String, char, u8, u16)> = Vec::new();

    let mut curr_slot = 'a';        
    for obj in inventory.iter() {
        let mut found = false;
        for j in 0..items.len() {
            if obj.item.as_ref().unwrap().stackable() && obj.get_fullname() == items[j].0 {
                items[j].2 += 1;
                found = true;
                break;
            }
        }

        if !found {
            items.push((obj.get_fullname(), curr_slot, 1, obj.item.as_ref().unwrap().value));
            curr_slot = (curr_slot as u8 + 1) as char;
        }
    }
    
    items
}

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
    let sbi = state.curr_sidebar_info(game_objs);
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

    let answer = gui.popup_menu(&name, &msg, &options, Some(&sbi));
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

fn check_grocer_inventory(state: &mut GameState, grocer_id: usize, game_objs: &mut GameObjects, ) {
    let curr_day = state.turn % 8640;
    let grocer = game_objs.get_mut(grocer_id).unwrap();
    let attitude = grocer.npc.as_ref().unwrap().attitude;
    let last_inventory_day = grocer.npc.as_ref().unwrap().last_inventory % 8640;

    let mut objs = Vec::new();
    if attitude == Attitude::Stranger {
        for _ in 0..rand::thread_rng().gen_range(3, 6) {
            let t = Item::get_item(game_objs, "torch").unwrap();
            objs.push(t);
        }
        for _ in 0..rand::thread_rng().gen_range(1, 3) {
            let t = Item::get_item(game_objs, "wineskin").unwrap();
            objs.push(t);
        }

        let grocer = game_objs.get_mut(grocer_id).unwrap();
        grocer.npc.as_mut().unwrap().inventory = objs;
    } else if curr_day - last_inventory_day >= 2  {
        // update inventory
    }
}

fn get_item_from_invetory(grocer_id: usize, name: &str, game_objs: &mut GameObjects) -> Option<GameObject> {
    let shopkeeper = game_objs.get_mut(grocer_id).unwrap();
    let inventory = &mut shopkeeper.npc.as_mut().unwrap().inventory;
    for j in 0..inventory.len() {
        if inventory[j].name == name {
            let item = inventory.remove(j);
            return Some(item);
        }
    }

    None
}

pub fn talk_to_grocer(state: &mut GameState, grocer_id: usize, game_objs: &mut GameObjects,
        dialogue: &DialogueLibrary, gui: &mut GameUI) {
    check_grocer_inventory(state, grocer_id, game_objs);
    let grocer = game_objs.get_mut(grocer_id).unwrap();

    let mut extra_info = HashMap::new();
    extra_info.insert("#goods#".to_string(), "adventuring supplies".to_string());
    let mut msg = grocer.npc.as_mut().unwrap().talk_to(state, dialogue, grocer.location, Some(extra_info));
    state.add_to_msg_history(&msg);


    loop {
        let sbi = state.curr_sidebar_info(game_objs);
        let grocer = game_objs.get_mut(grocer_id).unwrap();
        let inv = &grocer.npc.as_ref().unwrap().inventory;
        let menu_items = inventory_menu(&inv);
        let options: HashSet<char> = menu_items.iter().map(|i| i.1).collect();
        if menu_items.is_empty() {
            msg.push_str("\n\nI seem to be all out of stock!");

            let name = format!("{}, the grocer", grocer.get_npc_name(true).capitalize());
            gui.popup_msg(&name, &msg, Some(&sbi));
            break;
        } 
            
        let mut store_menu = msg.clone();
        store_menu.push_str("\n\nWhat would you like:\n");        
        for item in &menu_items {
            store_menu.push('\n');
            let mut s = format!("{}) {}", item.1, item.0);
            if item.2 > 1 {
                s.push_str(" (");
                s.push_str(&item.2.to_string());
                s.push_str("), ");
                s.push_str(&item.3.to_string());
                s.push_str("gp each")
            }
            else {
                s.push_str(", ");
                s.push_str(&item.3.to_string());
                s.push_str("gp");
            }
            store_menu.push_str(&s);            
        }
        
        let name = format!("{}, the grocer", grocer.get_npc_name(true).capitalize());
        if let Some(answer) = gui.popup_menu(&name, &store_menu, &options, Some(&sbi)) {
            for item in &menu_items {
                if item.1 == answer {                    
                    let p = game_objs.player_details();
                    if p.purse < item.3 as u32 {
                        gui.popup_msg(&name, "Hey! You can't afford that!", Some(&sbi));
                    } else {
                        let obj = get_item_from_invetory(grocer_id, &item.0, game_objs).unwrap();
                        let p = game_objs.player_details();
                        p.purse -= item.3 as u32;
                        p.add_to_inv(obj);
                    }
                }
            }          
        } else {
            break;
        }                
    }
}