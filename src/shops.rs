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

use super::{GameState, Message, Status};
use crate::effects;
use crate::npc::Attitude;
use crate::game_obj::{GameObject, Person};
use crate::dialogue::DialogueLibrary;
use crate::display::GameUI;
use crate::game_obj::{Ability, GameObjectDB, GameObjects};
use crate::items::{Item, ItemType};
use crate::util::StringUtils;

// This is similar to but not quite the same as the player inventory tool
fn inventory_menu(inventory: &Vec<GameObjects>) -> Vec<(String, char, u8, u16)> {    
    let mut items: Vec<(String, char, u8, u16)> = Vec::new();

    let mut curr_slot = 'a';        
    for obj in inventory.iter() {
        let mut found = false;
        for j in 0..items.len() {
            let stackable = if let GameObjects::Item(item) = obj {
                item.stackable()
            } else {
                false
            };
            if stackable && obj.get_fullname() == items[j].0 {
                items[j].2 += 1;
                found = true;
                break;
            }
        }

        if !found {
            let name = obj.get_fullname();
            let value = if let GameObjects::Item(item) = obj {
                item.value
            } else {
                0
            };
            items.push((name, curr_slot, 1, value));
            curr_slot = (curr_slot as u8 + 1) as char;
        }
    }
    
    items
}

// Is it worth preventing the character from renting a room if it's early in the day?
// Check in isn't until 3:00pm?
fn rent_room(state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    if let Some(GameObjects::Player(player)) = game_obj_db.get_mut(0) {
        if player.purse < 10 {
            state.msg_queue.push_back(Message::info("You can't afford a room in this establishment."));
            return;
        }

        player.purse -= 10;
        
        let checkout = state.turn + 2880; // renting a room is basically passing for 8 hours
        effects::add_status(player, Status::RestAtInn(checkout));

        state.msg_queue.push_back(Message::info("You check in."));
    }
}

// Eventually, having a drink will incease the player's verve (ie., the emotional readiness for dungeoneering)
// and/or possibly get them tipsy if they indulge too much.
fn buy_drink(state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    if let Some(GameObjects::Player(p)) = game_obj_db.get_mut(0) {
        if p.purse == 0 {
            state.msg_queue.push_back(Message::info("\"Hey this isn't a charity!\""));
        } else {
            p.purse -= 1;
            // more drink types eventually?
            state.msg_queue.push_back(Message::info("You drink a refreshing ale."));            
        }
    }
}

fn fill_flask(state: &mut GameState, game_obj_db: &mut GameObjectDB) {
    let player = game_obj_db.player().unwrap();

    let mut full = false;
    for obj in &mut player.inventory {
        if let GameObjects::Item(item) = obj {
            if item.item_type == ItemType::Bottle {
                if item.charges == 2 {
                    full = true;
                } else if player.purse < 2 {
                    state.msg_queue.push_back(Message::info("\"You're a bit short, I'm afraid.\""));
                    return;
                } else {
                    item.charges = 2;
                    player.purse -= 2;
                    let s = if rand::thread_rng().gen_range(0, 2) == 0 {
                        "\"There you are! Enjoy!\""
                    } else {
                        "\"Please drink and adventure responsibly!\""
                    };
                    state.msg_queue.push_back(Message::info(&s));
                    return;
                }
            }
        }        
    }

    let s = if full {
        "\"Yours is already full!\""
    } else {
        "\"You don't have anything to carry it in.\""
    };
    state.msg_queue.push_back(Message::info(&s));
}

fn buy_round(state: &mut GameState, game_obj_db: &mut GameObjectDB, patrons: &Vec<usize>) {
    let player = game_obj_db.player().unwrap();

    if player.purse < patrons.len() as u32 {
        state.msg_queue.push_back(Message::info("You can't afford to pay for everyone!"));
        return;
    } 

    state.msg_queue.push_back(Message::info("You buy a round for everyone in the bar."));
    player.purse -= patrons.len() as u32;

    let mut made_friend = false;
        
    for npc_id in patrons {
        let p = game_obj_db.player().unwrap();
        let persuasion = p.ability_check(Ability::Chr);

        if persuasion >= 13 {
            let patron = game_obj_db.get_mut(*npc_id).unwrap();
            if let GameObjects::NPC(npc) = patron {
                npc.attitude = Attitude::Friendly;
                made_friend = true;
            }
        }
    }

    if made_friend {
        state.msg_queue.push_back(Message::info("\"Cheers!\""));
    }
}

fn inn_patrons(state: &GameState, game_obj_db: &mut GameObjectDB, innkeeper_id: usize) -> Vec<usize> {
    let mut patrons = Vec::new();
    if let Some(buildings) = &state.world_info.town_buildings {
        for sq in buildings.tavern.iter() {
            if let Some(npc_id) = game_obj_db.npc_at(&sq) {
                patrons.push(npc_id);
            }
        }
    }

    patrons.retain(|p| *p != innkeeper_id);

    patrons
}

pub fn talk_to_innkeeper(state: &mut GameState, innkeeper_id: usize, game_obj_db: &mut GameObjectDB, 
        dialogue: &DialogueLibrary, gui: &mut GameUI) {
    let sbi = state.curr_sidebar_info(game_obj_db);
    let patrons = inn_patrons(state, game_obj_db, innkeeper_id);
    let npc = game_obj_db.get_mut(innkeeper_id).unwrap();
    let mut ei = HashMap::new();
    let mut msg = String::from("");
    if let GameObjects::NPC(npc) = npc {
        npc.talk_to(state, dialogue, &mut ei);
    }

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

    let name = format!("{}, the innkeeper", npc.get_fullname().capitalize());
    
    let answer = gui.popup_menu(&name, &msg, &options, Some(&sbi));
    if let Some(ch) = answer {
        if ch == 'a' {
            buy_drink(state, game_obj_db);
        } else if ch == 'b' {
            rent_room(state, game_obj_db);
        } else if ch == 'c' {
            fill_flask(state, game_obj_db);
        } else if ch == 'd' {
            buy_round(state, game_obj_db, &patrons);
        }
    } else {
        let x = rand::thread_rng().gen_range(0, 3);
        let s = if x == 0 {
            "\"Nevermind.\""
        } else if x == 1 {
            "\"No loitering.\""
        } else {
            "\"No outside food or drink.\""
        };
        state.msg_queue.push_back(Message::info(&s));        
    }
}

fn check_smith_inventory(state: &mut GameState, smith_id: usize, game_obj_db: &mut GameObjectDB, ) {
    let curr_day = state.turn / 8640;
    let smith = game_obj_db.get_mut(smith_id).unwrap();
    let (first_inventory, last_inventory_day) = if let GameObjects::NPC(npc) = smith {
        (npc.attitude == Attitude::Stranger, npc.last_inventory / 8640)
    } else {
        (true, 0)
    };

    let mut objs = Vec::new();
    if first_inventory {
        // The initial inventory when the player first meets the shopkeeper        
        let ls = Item::get_item(game_obj_db, "longsword").unwrap();
        objs.push(ls);
        
        for _ in 0..rand::thread_rng().gen_range(1, 4) {
            let d = Item::get_item(game_obj_db, "dagger").unwrap();
            objs.push(d);
        }

        if rand::thread_rng().gen_range(0, 3) == 0 {
            let ts = Item::get_item(game_obj_db, "two-handed sword").unwrap();
            objs.push(ts);
        }
        
        if rand::thread_rng().gen_range(0, 2) == 0 {
            let s = Item::get_item(game_obj_db, "spear").unwrap();
            objs.push(s);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let ch = Item::get_item(game_obj_db, "chainmail").unwrap();
            objs.push(ch);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let sh = Item::get_item(game_obj_db, "shield").unwrap();
            objs.push(sh);
        }

        let smith = game_obj_db.get_mut(smith_id).unwrap();

        if let GameObjects::NPC(npc) = smith {
            npc.inventory = objs;
            npc.last_inventory = state.turn;
        }
    } else if curr_day - last_inventory_day >= 2  {        
        // Update inventory every couple of days.
        
        // First, generate the new stock
        let mut new_stock = Vec::new();

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let ls = Item::get_item(game_obj_db, "longsword").unwrap();
            new_stock.push(ls);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let d = Item::get_item(game_obj_db, "dagger").unwrap();
            new_stock.push(d);
        }

        if rand::thread_rng().gen_range(0, 3) == 0 {
            let ts = Item::get_item(game_obj_db, "two-handed sword").unwrap();
            new_stock.push(ts);
        }
        
        if rand::thread_rng().gen_range(0, 2) == 0 {
            let s = Item::get_item(game_obj_db, "spear").unwrap();
            new_stock.push(s);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let ch = Item::get_item(game_obj_db, "chainmail").unwrap();
            new_stock.push(ch);
        }

        if rand::thread_rng().gen_range(0, 2) == 0 {
            let sh = Item::get_item(game_obj_db, "shield").unwrap();
            new_stock.push(sh);
        }

        // For any item in their current inventory, there's a 25% chance it's been purchases while the 
        // player's been away
        let smith = game_obj_db.get_mut(smith_id).unwrap();
        if let GameObjects::NPC(npc) = smith {        
            let mut to_remove = Vec::new();
            for j in 0..npc.inventory.len() {
                if rand::thread_rng().gen_range(0.0, 1.0) <= 0.2 {
                    to_remove.push(j);
                }
            }
            to_remove.sort();
            to_remove.reverse();
            for j in to_remove {
                npc.inventory.remove(j);
            }
        
            npc.last_inventory = state.turn;            
            for obj in new_stock {
                npc.inventory.push(obj);
            }
        }
    }
}

fn check_grocer_inventory(state: &mut GameState, grocer_id: usize, game_obj_db: &mut GameObjectDB, ) {
    let curr_day = state.turn / 8640;
    let grocer = game_obj_db.get_mut(grocer_id).unwrap();
    let (first_inventory, last_inventory_day) = if let GameObjects::NPC(npc) = grocer {
        (npc.attitude == Attitude::Stranger, npc.last_inventory / 8640)
    } else {
        (true, 0)
    };

    let mut objs = Vec::new();
    if first_inventory {
        // The initial inventory when the player first meets the shopkeeper
        for _ in 0..rand::thread_rng().gen_range(3, 6) {
            let t = Item::get_item(game_obj_db, "torch").unwrap();
            objs.push(t);
        }
        for _ in 0..rand::thread_rng().gen_range(1, 3) {
            let w = Item::get_item(game_obj_db, "wineskin").unwrap();
            objs.push(w);
        }
        for _ in 0..rand::thread_rng().gen_range(1, 4) {
            let p = Item::get_item(game_obj_db, "potion of healing").unwrap();
            objs.push(p);
        }
        let grocer = game_obj_db.get_mut(grocer_id).unwrap();
        if let GameObjects::NPC(npc) = grocer {
            npc.inventory = objs;
            npc.last_inventory = state.turn;
        }        
    } else if curr_day - last_inventory_day >= 2  {        
        // Update inventory every couple of days.
        
        // Generate some new stock for the shopkeeper
        let mut new_stock = Vec::new();
        for _ in 0..rand::thread_rng().gen_range(0, 4) {
            let t = Item::get_item(game_obj_db, "torch").unwrap();
            new_stock.push(t);
        }
        for _ in 0..rand::thread_rng().gen_range(0, 2) {
            let w = Item::get_item(game_obj_db, "wineskin").unwrap();
            new_stock.push(w);
        }
        for _ in 0..rand::thread_rng().gen_range(1, 4) {
            let p = Item::get_item(game_obj_db, "potion of healing").unwrap();
            new_stock.push(p);
        }
        
        // For any item in their current inventory, there's a 25% chance it's been purchases while the 
        // player's been away
        let grocer = game_obj_db.get_mut(grocer_id).unwrap();
        if let GameObjects::NPC(npc) = grocer {
            let mut to_remove = Vec::new();
            for j in 0..npc.inventory.len() {
                if rand::thread_rng().gen_range(0.0, 1.0) <= 0.2 {
                    to_remove.push(j);
                }
            }
            to_remove.sort();
            to_remove.reverse();
            for j in to_remove {
                npc.inventory.remove(j);
            }

            npc.last_inventory = state.turn;
            for obj in new_stock {
                npc.inventory.push(obj);
            }
        }
    }
}

fn get_item_from_invetory(npc_id: usize, name: &str, game_obj_db: &mut GameObjectDB) -> Option<GameObjects> {
    let shopkeeper = game_obj_db.get_mut(npc_id).unwrap();
    if let GameObjects::NPC(npc) = shopkeeper {
        for j in 0..npc.inventory.len() {
            if npc.inventory[j].get_fullname() == name {
                let item = npc.inventory.remove(j);
                return Some(item);
            }
        }
    }

    None
}

pub fn talk_to_grocer(state: &mut GameState, grocer_id: usize, game_obj_db: &mut GameObjectDB, dialogue: &DialogueLibrary, gui: &mut GameUI) {
    let sbi = state.curr_sidebar_info(game_obj_db);
    check_grocer_inventory(state, grocer_id, game_obj_db);
    let grocer = game_obj_db.get_mut(grocer_id).unwrap();
    let mut msg = "".to_string();

    if let GameObjects::NPC(npc) = grocer {
        let mut extra_info = HashMap::new();
        extra_info.insert("#goods#".to_string(), "adventuring supply".to_string());
        msg = npc.talk_to(state, dialogue, &mut extra_info);

        if let Some(agenda) = npc.curr_agenda_item(state) {
            if agenda.label != "working" {
                let name = format!("{}, the grocer", npc.npc_name(true).capitalize());
                gui.popup_msg(&name, &msg, Some(&sbi));
                return;
            }
        }
    }
    
    let mut made_purchase = false;
    loop {
        let sbi = state.curr_sidebar_info(game_obj_db);
        let grocer = game_obj_db.get_mut(grocer_id).unwrap();
        let menu_items = if let GameObjects::NPC(npc) = grocer {
            inventory_menu(&npc.inventory)
        } else {
            Vec::new()
        };
        
        let options: HashSet<char> = menu_items.iter().map(|i| i.1).collect();
        if menu_items.is_empty() {
            msg.push_str("\n\nI seem to be all out of stock!");

            let name = format!("{}, the grocer", grocer.get_fullname().capitalize());
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
        
        let name = format!("{}, the grocer", grocer.get_fullname().capitalize());
        if let Some(answer) = gui.popup_menu(&name, &store_menu, &options, Some(&sbi)) {
            for item in &menu_items {
                if item.1 == answer {                    
                    let p = game_obj_db.player().unwrap();
                    if p.purse < item.3 as u32 {
                        gui.popup_msg(&name, "Hey! You can't afford that!", Some(&sbi));
                    } else {
                        let obj = get_item_from_invetory(grocer_id, &item.0, game_obj_db).unwrap();
                        let p = game_obj_db.player().unwrap();
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
        state.msg_queue.push_back(Message::info("\"Thank you for supporting small businesses!\""));
    }
}

// For when I implement rust/corrosion
fn repair_gear(state: &mut GameState, _game_obj_db: &mut GameObjectDB, _gui: &mut GameUI) {
    state.msg_queue.push_back(Message::info("\"Hmm none of your equipment needs fixing right now.\""));      
}

fn purchase_from_smith(state: &mut GameState, smith_id: usize, name: String, preamble: &str, game_obj_db: &mut GameObjectDB, gui: &mut GameUI) -> bool {
    let mut msg = preamble.to_string();
    let mut made_purchase = false;
    loop {
        let sbi = state.curr_sidebar_info(game_obj_db);
        let smith = game_obj_db.get_mut(smith_id).unwrap();
        let menu_items = if let GameObjects::NPC(npc) = smith {
            inventory_menu(&npc.inventory)
        } else {
            Vec::new()
        };
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
                    let p = game_obj_db.player().unwrap();
                    if p.purse < item.3 as u32 {
                        gui.popup_msg(&name, "Hey! You can't afford that!", Some(&sbi));
                    } else {
                        let obj = get_item_from_invetory(smith_id, &item.0, game_obj_db).unwrap();
                        let p = game_obj_db.player().unwrap();
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

pub fn talk_to_smith(state: &mut GameState, smith_id: usize, game_obj_db: &mut GameObjectDB, dialogue: &DialogueLibrary, gui: &mut GameUI) {
    let sbi = state.curr_sidebar_info(game_obj_db);
    check_smith_inventory(state, smith_id, game_obj_db);
    let smith = game_obj_db.get_mut(smith_id).unwrap();
    let mut msg = "".to_string();
    let mut name = "".to_string();
    if let GameObjects::NPC(npc) = smith {
        name = format!("{}, the smith", npc.npc_name(true).capitalize());
        let mut extra_info = HashMap::new();
        msg = npc.talk_to(state, dialogue, &mut extra_info);

        if let Some(agenda) = npc.curr_agenda_item(state) {
            if agenda.label != "working" {            
                let name = format!("{}, the smith", name);
                gui.popup_msg(&name, &msg, Some(&sbi));
                return;
            }
        }
    }
    let preamble = msg.clone();
    
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
            made_purchase = purchase_from_smith(state, smith_id, name.clone(), &preamble, game_obj_db, gui);
        } else if ch == 'b' {
            repair_gear(state, game_obj_db, gui);
        } 
    } else {
        state.msg_queue.push_back(Message::info("Never mind."));
    }

    if made_purchase {
        state.msg_queue.push_back(Message::info("\"I hope that serves you well!\""));
    }
}