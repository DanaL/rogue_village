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
use crate::{actor::Attitude, items::ItemType};
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
            if obj.item.as_ref().unwrap().stackable() && obj.name == items[j].0 {
                items[j].2 += 1;
                found = true;
                break;
            }
        }

        if !found {
            let name = obj.name.clone();
            items.push((name, curr_slot, 1, obj.item.as_ref().unwrap().value));
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

fn fill_flask(state: &mut GameState, game_objs: &mut GameObjects) {
    let player = game_objs.player_details();

    let mut full = false;
    for obj in &mut player.inventory {
        let mut item = obj.item.as_mut().unwrap();
        if item.item_type == ItemType::Bottle {
            if item.charges == 2 {
                full = true;
            } else if player.purse < 2 {
                state.write_msg_buff("\"You're a bit short, I'm afraid.\"");
                return;
            } else {
                item.charges = 2;
                player.purse -= 2;
                if rand::thread_rng().gen_range(0, 2) == 0 {
                    state.write_msg_buff("\"There you are! Enjoy!\"");
                } else {
                    state.write_msg_buff("\"Please drink and adventure responsibly!\"");
                }
                return;
            }
        }
    }

    if full {
        state.write_msg_buff("\"Yours is already full!\"");
    } else {
        state.write_msg_buff("\"You don't have anything to carry it in.\"");
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
    let mut ei = HashMap::new();
    let mut msg = npc.npc.as_mut().unwrap().talk_to(state, dialogue, npc.location, &mut ei);
    state.add_to_msg_history(&msg);
    
    let mut options: HashSet<char> = vec!['a', 'b', 'c'].into_iter().collect();
    msg.push('\n');
    msg.push('\n');
    msg.push_str("a) buy a drink (1$)\n");
    msg.push_str("b) rent a room (10$)\n");
    msg.push_str("c) fill a wineskin (2$)\n");
    if !patrons.is_empty() {
        let s = format!("d) buy a round for the bar ({}$)", patrons.len());
        msg.push_str(&s);
        options.insert('d');
    }

    let name = format!("{}, the innkeeper", npc.get_npc_name(true).capitalize());
    
    let answer = gui.popup_menu(&name, &msg, &options, Some(&sbi));
    if let Some(ch) = answer {
        if ch == 'a' {
            buy_drink(state, game_objs);
        } else if ch == 'b' {
            rent_room(state, game_objs);
        } else if ch == 'c' {
            fill_flask(state, game_objs);
        } else if ch == 'd' {
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

fn check_smith_inventory(state: &mut GameState, smith_id: usize, game_objs: &mut GameObjects, ) {
    let curr_day = state.turn / 8640;
    let smith = game_objs.get_mut(smith_id).unwrap();
    let attitude = smith.npc.as_ref().unwrap().attitude;
    let last_inventory_day = smith.npc.as_ref().unwrap().last_inventory / 8640;

    let mut objs = Vec::new();
    if attitude == Attitude::Stranger {
        // The initial inventory when the player first meets the shopkeeper        
        let ls = Item::get_item(game_objs, "longsword").unwrap();
        objs.push(ls);
        
        for _ in 0..rand::thread_rng().gen_range(1, 4) {
            let d = Item::get_item(game_objs, "dagger").unwrap();
            objs.push(d);
        }

        if rand::thread_rng().gen_range(0, 3) == 0 {
            let ts = Item::get_item(game_objs, "two-handed sword").unwrap();
            objs.push(ts);
        }
        
        if rand::thread_rng().gen_range(0, 2) == 0 {
            let s = Item::get_item(game_objs, "spear").unwrap();
            objs.push(s);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let ch = Item::get_item(game_objs, "chainmail").unwrap();
            objs.push(ch);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let sh = Item::get_item(game_objs, "shield").unwrap();
            objs.push(sh);
        }

        let smith = game_objs.get_mut(smith_id).unwrap();
        smith.npc.as_mut().unwrap().inventory = objs;
        smith.npc.as_mut().unwrap().last_inventory = state.turn;
    } else if curr_day - last_inventory_day >= 2  {        
        // Update inventory every couple of days.
        
        // For any item in their current inventory, there's a 25% chance it's been purchases while the 
        // player's been away
        let smith = game_objs.get_mut(smith_id).unwrap();
        let inv = &mut smith.npc.as_mut().unwrap().inventory;
        let mut to_remove = Vec::new();
        for j in 0..inv.len() {
            if rand::thread_rng().gen_range(0.0, 1.0) <= 0.2 {
                to_remove.push(j);
            }
        }
        to_remove.sort();
        to_remove.reverse();
        for j in to_remove {
            inv.remove(j);
        }

        let mut new_stock = Vec::new();

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let ls = Item::get_item(game_objs, "longsword").unwrap();
            new_stock.push(ls);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let d = Item::get_item(game_objs, "dagger").unwrap();
            new_stock.push(d);
        }

        if rand::thread_rng().gen_range(0, 3) == 0 {
            let ts = Item::get_item(game_objs, "two-handed sword").unwrap();
            new_stock.push(ts);
        }
        
        if rand::thread_rng().gen_range(0, 2) == 0 {
            let s = Item::get_item(game_objs, "spear").unwrap();
            new_stock.push(s);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let ch = Item::get_item(game_objs, "chainmail").unwrap();
            new_stock.push(ch);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let sh = Item::get_item(game_objs, "shield").unwrap();
            new_stock.push(sh);
        }

        let smith = game_objs.get_mut(smith_id).unwrap();
        smith.npc.as_mut().unwrap().last_inventory = state.turn;
        for obj in new_stock {
            smith.npc.as_mut().unwrap().inventory.push(obj);
        }
    }
}

fn check_grocer_inventory(state: &mut GameState, grocer_id: usize, game_objs: &mut GameObjects, ) {
    let curr_day = state.turn / 8640;
    let grocer = game_objs.get_mut(grocer_id).unwrap();
    let attitude = grocer.npc.as_ref().unwrap().attitude;
    let last_inventory_day = grocer.npc.as_ref().unwrap().last_inventory / 8640;

    let mut objs = Vec::new();
    if attitude == Attitude::Stranger {
        // The initial inventory when the player first meets the shopkeeper
        for _ in 0..rand::thread_rng().gen_range(3, 6) {
            let t = Item::get_item(game_objs, "torch").unwrap();
            objs.push(t);
        }
        for _ in 0..rand::thread_rng().gen_range(1, 3) {
            let w = Item::get_item(game_objs, "wineskin").unwrap();
            objs.push(w);
        }
        for _ in 0..rand::thread_rng().gen_range(1, 4) {
            let p = Item::get_item(game_objs, "potion of healing").unwrap();
            objs.push(p);
        }
        let grocer = game_objs.get_mut(grocer_id).unwrap();
        grocer.npc.as_mut().unwrap().inventory = objs;
        grocer.npc.as_mut().unwrap().last_inventory = state.turn;
    } else if curr_day - last_inventory_day >= 2  {        
        // Update inventory every couple of days.
        
        // For any item in their current inventory, there's a 25% chance it's been purchases while the 
        // player's been away
        let grocer = game_objs.get_mut(grocer_id).unwrap();
        let inv = &mut grocer.npc.as_mut().unwrap().inventory;
        let mut to_remove = Vec::new();
        for j in 0..inv.len() {
            if rand::thread_rng().gen_range(0.0, 1.0) <= 0.2 {
                to_remove.push(j);
            }
        }
        to_remove.sort();
        to_remove.reverse();
        for j in to_remove {
            inv.remove(j);
        }

        let mut new_stock = Vec::new();
        for _ in 0..rand::thread_rng().gen_range(0, 4) {
            let t = Item::get_item(game_objs, "torch").unwrap();
            new_stock.push(t);
        }
        for _ in 0..rand::thread_rng().gen_range(0, 2) {
            let w = Item::get_item(game_objs, "wineskin").unwrap();
            new_stock.push(w);
        }
        for _ in 0..rand::thread_rng().gen_range(1, 4) {
            let p = Item::get_item(game_objs, "potion of healing").unwrap();
            new_stock.push(p);
        }
        
        let grocer = game_objs.get_mut(grocer_id).unwrap();
        grocer.npc.as_mut().unwrap().last_inventory = state.turn;
        for obj in new_stock {
            grocer.npc.as_mut().unwrap().inventory.push(obj);
        }
    }
}

fn get_item_from_invetory(npc_id: usize, name: &str, game_objs: &mut GameObjects) -> Option<GameObject> {
    let shopkeeper = game_objs.get_mut(npc_id).unwrap();
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
    let sbi = state.curr_sidebar_info(game_objs);
    check_grocer_inventory(state, grocer_id, game_objs);
    let grocer = game_objs.get_mut(grocer_id).unwrap();
    
    let mut extra_info = HashMap::new();
    extra_info.insert("#goods#".to_string(), "adventuring supply".to_string());
    let mut msg = grocer.npc.as_mut().unwrap().talk_to(state, dialogue, grocer.location, &mut extra_info);
    state.add_to_msg_history(&msg);

    if let Some(agenda) = grocer.npc.as_ref().unwrap().curr_agenda_item(state) {
        if agenda.label != "working" {            
            let name = format!("{}, the grocer", grocer.get_npc_name(true).capitalize());
            gui.popup_msg(&name, &msg, Some(&sbi));
            return;
        }
    }

    let mut made_purchase = false;
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
                s.push_str("$ each")
            }
            else {
                s.push_str(", ");
                s.push_str(&item.3.to_string());
                s.push_str("$");
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
                        made_purchase = true;
                    }
                }
            }          
        } else {
            break;
        }                
    }

    if made_purchase {
        state.write_msg_buff("\"Thank you for supporting small businesses!\"");
    }
}

// For when I implement rust/corrosion
fn repair_gear(state: &mut GameState, _game_objs: &mut GameObjects) {
    state.write_msg_buff("\"Hmm none of your equipment needs fixing at the moment.\"");
}

fn purchase_from_smith(state: &mut GameState, smith_id: usize, name: String, preamble: &str, game_objs: &mut GameObjects, gui: &mut GameUI) -> bool {
    let mut msg = preamble.to_string();
    let mut made_purchase = false;
    loop {
        let sbi = state.curr_sidebar_info(game_objs);
        let smith = game_objs.get_mut(smith_id).unwrap();
        let inv = &smith.npc.as_ref().unwrap().inventory;
        let menu_items = inventory_menu(&inv);
        let options: HashSet<char> = menu_items.iter().map(|i| i.1).collect();
        if menu_items.is_empty() {
            msg.push_str("\n\nI seem to be all out of stock!");

            let name = format!("{}, the smith", name);
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
                s.push_str("$ each")
            }
            else {
                s.push_str(", ");
                s.push_str(&item.3.to_string());
                s.push_str("$");
            }
            store_menu.push_str(&s);            
        }
        
        if let Some(answer) = gui.popup_menu(&name, &store_menu, &options, Some(&sbi)) {
            for item in &menu_items {
                if item.1 == answer {                    
                    let p = game_objs.player_details();
                    if p.purse < item.3 as u32 {
                        gui.popup_msg(&name, "Hey! You can't afford that!", Some(&sbi));
                    } else {
                        let obj = get_item_from_invetory(smith_id, &item.0, game_objs).unwrap();
                        let p = game_objs.player_details();
                        p.purse -= item.3 as u32;
                        p.add_to_inv(obj);
                        made_purchase = true;
                    }
                }
            }          
        } else {
            break;
        }                
    }

    made_purchase
}

pub fn talk_to_smith(state: &mut GameState, smith_id: usize, game_objs: &mut GameObjects,
        dialogue: &DialogueLibrary, gui: &mut GameUI) {
    let sbi = state.curr_sidebar_info(game_objs);
    check_smith_inventory(state, smith_id, game_objs);
    let smith = game_objs.get_mut(smith_id).unwrap();
    let name = format!("{}, the smith", smith.get_npc_name(true).capitalize());

    let mut extra_info = HashMap::new();
    let mut msg = smith.npc.as_mut().unwrap().talk_to(state, dialogue, smith.location, &mut extra_info);
    let preamble = msg.clone();
    state.add_to_msg_history(&msg);

    if let Some(agenda) = smith.npc.as_ref().unwrap().curr_agenda_item(state) {
        if agenda.label != "working" {            
            let name = format!("{}, the smith", smith.get_npc_name(true).capitalize());
            gui.popup_msg(&name, &msg, Some(&sbi));
            return;
        }
    }

    // First choice, purchase or repair gear
    msg.push('\n');
    msg.push('\n');
    msg.push_str("a) see my wares\n");
    msg.push_str("b) repair your gear\n");
    
    let options: HashSet<char> = vec!['a', 'b'].into_iter().collect();
    let mut made_purchase = false;
    let answer = gui.popup_menu(&name, &msg, &options, Some(&sbi));
    if let Some(ch) = answer {
        if ch == 'a' {
            made_purchase = purchase_from_smith(state, smith_id, name.clone(), &preamble,
                game_objs, gui);
        } else if ch == 'b' {
            repair_gear(state, game_objs);
        } 
    } else {
        state.write_msg_buff("\"Nevermind.\"");
    }

    if made_purchase {
        state.write_msg_buff("\"I hope that serves you well!\"");
    }
}