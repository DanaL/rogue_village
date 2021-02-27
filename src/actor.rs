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

use rand::thread_rng;
use rand::Rng;
//use std::time::{Duration, Instant};

use super::{EventType, GameObject, GameObjects, GameState};

use crate::dialogue;
use crate::dialogue::DialogueLibrary;
use crate::display::LIGHT_GREY;
use crate::items::Item;
use crate::map::{Tile, DoorState};
use crate::pathfinding::find_path;
use crate::player::Player;
use crate::util;

#[derive(Clone, Debug)]
pub enum Venue {
    TownSquare,
    Tavern,
    Shrine,
    Favourite((i32, i32, i8)),
    Visit(i32),
}
#[derive(Clone, Debug)]
pub struct AgendaItem {
    pub from: (u16, u16),
    pub to: (u16, u16),
    pub priority: u8,
    pub place: Venue,
}

impl AgendaItem {
    pub fn new(from: (u16, u16), to: (u16, u16), priority: u8, place: Venue) -> AgendaItem {
        AgendaItem { from, to, priority, place, }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Copy)]
pub enum Attitude {
    Stranger,
    Indifferent,
    Friendly,
    Hostile,
}

pub trait Actor {
    fn act(&mut self, state: &mut GameState, game_objects: &mut GameObjects);
    fn talk_to(&mut self, state: &mut GameState, player: &Player, dialogue: &DialogueLibrary) -> String;
}


#[derive(Clone, Debug)]
pub struct BasicStats {
    pub name: String,
	pub max_hp: u8,
	pub curr_hp: u8,
	pub location: (i32, i32, i8),
    pub ch: char,
    pub color: (u8, u8, u8),
    pub attitude: Attitude,
}

impl BasicStats {
    pub fn new(name: String, max_hp: u8, curr_hp: u8, location: (i32, i32, i8), ch: char, color: (u8, u8, u8), attitude: Attitude) -> BasicStats {
        let bs = BasicStats {
            name, max_hp, curr_hp, location, ch, color, attitude,
        };  

        bs
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Move((i32, i32, i8)),
    OpenDoor((i32, i32, i8)),
    CloseDoor((i32, i32, i8)),
}

#[derive(Clone, Debug)]
pub struct Villager {
    pub stats: BasicStats,	
    pub facts_known: Vec<usize>,
    pub greeted_player: bool,
    pub home_id: usize,
    pub plan: VecDeque<Action>,
    pub voice: String,
    pub schedule: Vec<AgendaItem>,
    pub object_id: usize,
}

impl Villager {
    pub fn new(name: String, location: (i32, i32, i8), home_id: usize, voice: &str, object_id: usize) -> Villager {
        Villager { stats: BasicStats::new(name, 8,  8, location,  '@',  LIGHT_GREY, Attitude::Stranger), 
            facts_known: Vec::new(), greeted_player: false, home_id,
            plan: VecDeque::new(), voice: String::from(voice), schedule: Vec::new(), object_id
        }
    }

    // I should be able to move calc_plan_to_move, try_to_move_to_loc, etc to generic
    // places for all Villager types since they'll be pretty same-y. The differences
    // will be in how NPCs set their plans/schedules. 
    fn calc_plan_to_move(&mut self, state: &GameState, goal: (i32, i32, i8), stop_before: bool) {
        if self.plan.len() == 0 {
            let mut passable = HashMap::new();
            passable.insert(Tile::Grass, 1.0);
            passable.insert(Tile::Dirt, 1.0);
            passable.insert(Tile::Tree, 1.0);
            passable.insert(Tile::Door(DoorState::Open), 1.0);
            passable.insert(Tile::Door(DoorState::Closed), 2.0);
            passable.insert(Tile::Door(DoorState::Broken), 1.0);
            passable.insert(Tile::Door(DoorState::Locked), 2.5);
            passable.insert(Tile::StoneFloor, 1.0);
            passable.insert(Tile::Floor, 1.0);

            let mut path = find_path(&state.map, stop_before, self.stats.location.0, self.stats.location.1, 
                self.stats.location.2, goal.0, goal.1, 50, &passable);
            
            path.pop(); // first square in path is the start location
            while path.len() > 0 {
                let sq = path.pop().unwrap();
                self.plan.push_back(Action::Move((sq.0, sq.1, self.stats.location.2)));
            }
        }
    }

    fn try_to_move_to_loc(&mut self, loc: (i32, i32, i8), state: &mut GameState, game_objs: &mut GameObjects) {        
        // if npcs.contains_key(&loc) || state.player_loc == loc {
        //     state.write_msg_buff("\"Excuse me.\"");
        //     self.plan.push_front(Action::Move(loc));
        // } else if state.map[&loc] == Tile::Door(DoorState::Closed) {
        //     self.plan.push_front(Action::Move(loc));
        //     self.open_door(loc, state);
        // } else {
        //     // Villagers are fairly polite. If they go through a door, they will close it after them, 
        //     // just like their parents said they should.
        //     // There's a flaw here in that at the moment, villagers never abandon their plans. So, if, say
        //     // a villager is going through a door and someone is following right behind, they will wait for
        //     // the other to move so they can close the door, but the other will want to move into the 
        //     // building and they'll be deadlocked forever.
        //     if let Tile::Door(DoorState::Open) = state.map[&self.get_location()] {
        //         self.plan.push_front(Action::CloseDoor(self.get_location()));                
        //     }
        //     self.stats.location = loc;
        // }
    }

    fn open_door(&mut self, loc: (i32, i32, i8), state: &mut GameState) {
        if self.stats.attitude == Attitude::Stranger {
            state.write_msg_buff("The villager opens the door.");
        } else {
            let s = format!("{} opens the door.", self.get_fullname());
            state.write_msg_buff(&s);
        }
        state.map.insert(loc, Tile::Door(DoorState::Open));
    }

    fn close_door(&mut self, loc: (i32, i32, i8), state: &mut GameState, game_objs: &mut GameObjects) {
        // if npcs.contains_key(&loc) || loc == state.player_loc {
        //     state.write_msg_buff("Please don't stand in the doorway.");
        //     self.plan.push_front(Action::CloseDoor(loc));
        // } else {
        //     if let Tile::Door(DoorState::Open) = state.map[&loc] {
        //         if self.stats.attitude == Attitude::Stranger {
        //             state.write_msg_buff("The villager closes the door.");
        //         } else {
        //             let s = format!("{} closes the door.", self.get_name());
        //             state.write_msg_buff(&s);
        //         }
        //         state.map.insert(loc, Tile::Door(DoorState::Closed));
        //     }
        // }
    }

    fn follow_plan(&mut self, state: &mut GameState, game_objs: &mut GameObjects) {
        let action = self.plan.pop_front().unwrap();
        match action {
            Action::Move(loc) => self.try_to_move_to_loc(loc, state, game_objs),
            Action::OpenDoor(loc) => self.open_door(loc, state),
            Action::CloseDoor(loc) => self.close_door(loc, state, game_objs),
        }
    }

    // fn is_home_open(&self, map: &Map) -> bool {
    //     match self.entrance_location(map) {
    //         Some(loc) => 
    //             if map[&loc] == Tile::Door(DoorState::Open) {
    //                 true
    //             } else {
    //                 false
    //             },
    //         _ => false
    //     }        
    // }

    fn idle_behaviour(&mut self, state: &GameState) {
        // If the NPC doesn't need to move anywhere, just pick an adjacent square to step to sometimes.
        // (Maybe eventually if they are adjacent to another NPC, have them make small talk?)
        if thread_rng().gen_range(0.0, 1.0) < 0.33 {
            let j = thread_rng().gen_range(0, util::ADJ.len()) as usize;
            let d = util::ADJ[j];
            let adj = (self.stats.location.0 + d.0, self.stats.location.1 + d.1, self.stats.location.2);
            if state.map[&adj].passable_dry_land() {
                self.calc_plan_to_move(state, adj, false);
            }
        }
    }

    fn check_agenda_item(&mut self, state: &GameState, item: &AgendaItem) {        
        match item.place {
            Venue::Tavern => {
                let tavern = &state.world_info.town_buildings.as_ref().unwrap().tavern;
                if !in_location(state, self.get_location(), &tavern, true) {
                    self.go_to_place(state, tavern);
                } else {
                    self.idle_behaviour(state);
                }
            },
            Venue::TownSquare => {
                let ts = &state.world_info.town_square;
                if !in_location(state, self.get_location(), ts, false) {
                    self.go_to_place(state, ts);
                } else {
                    self.idle_behaviour(state);
                }
            },
            _ => {
                // Eventually I'll implement the other venues...
            },
        }
    }

    fn check_schedule(&mut self, state: &GameState) {
        let ct = state.curr_time();
        let minutes = ct.0 * 60 + ct.1;
        
        // Select the current, highest priority agenda item from the schedule
        let mut items: Vec<&AgendaItem> = self.schedule.iter()
                     .filter(|i| i.from.0 * 60 + i.from.1 <= minutes && minutes <= i.to.0 * 60 + i.to.1)
                     .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        if items.len() == 0 {
            // The default behaviour is to go home if nothing on the agenda.
            let b = &state.world_info.town_buildings.as_ref().unwrap();
            if !in_location(state, self.get_location(), &b.homes[self.home_id], true) {
                self.go_to_place(state, &b.homes[self.home_id]);
            } else {
                self.idle_behaviour(state);
            }            
        } else {
            let item = &items[0].clone();
            self.check_agenda_item(state, item);
        }
    }

    // Generally, when I have an NPC go a building/place, I assume it doesn't matter too much if 
    // they go to specific square inside it, so just pick any one of them.
    fn go_to_place(&mut self, state: &GameState, sqs: &HashSet<(i32, i32, i8)>) {
        let j = thread_rng().gen_range(0, &sqs.len());
        let goal_loc = &sqs.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
        self.calc_plan_to_move(state, *goal_loc, false);
    }
}

// Eventually I'll be able to reuse a bunch of this behaviour code for all Villagers
// (I hope) without cutting and pasting everywhere.
impl Actor for Villager {
    fn act(&mut self, state: &mut GameState, game_objs: &mut GameObjects) {
        // It's a mayoral duty to greet newcomers to town
        /*
        let pl = state.player_loc;
        
        if !self.greeted_player && pl.2 == self.stats.location.2 && util::distance(pl.0, pl.1, self.stats.location.0,self.stats.location.1) <= 4.0 {                 
            let s = format!("Hello stranger, welcome to {}!", state.world_info.town_name);
            state.write_msg_buff(&s);
            self.greeted_player = true;
        }
        */

        if self.plan.len() > 0 {
            self.follow_plan(state, game_objs);            
        } else {
            self.check_schedule(state);
        } 
    }

    fn talk_to(&mut self, state: &mut GameState, player: &Player, dialogue: &DialogueLibrary) -> String {
        let line = dialogue::parse_voice_line(&dialogue::pick_voice_line(dialogue, &self.voice, self.stats.attitude), &state.world_info, player, &self.stats);        
        if self.stats.attitude == Attitude::Stranger {
            // Perhaps a charisma check to possibly jump straight to friendly?
            self.stats.attitude = Attitude::Indifferent;
        }

        line
    }
}

#[derive(Clone, Debug)]
pub struct SimpleMonster {
    pub stats: BasicStats,    
}

impl SimpleMonster {
    pub fn new(name: String, location:( i32, i32, i8), ch: char, color: (u8, u8, u8)) -> SimpleMonster {
        SimpleMonster { stats: BasicStats::new(name,  8,  8, location, ch, color, Attitude::Hostile) }
    }
}

impl Actor for SimpleMonster {
    fn act(&mut self, _state: &mut GameState, game_objs: &mut GameObjects) {
        
    }

    fn talk_to(&mut self, _state: &mut GameState, _player: &Player, _dialogue: &DialogueLibrary) -> String {
        format!("The {} growls at you!", self.stats.name)
    }
}

fn in_location(state: &GameState, loc: (i32, i32, i8), sqs: &HashSet<(i32, i32, i8)>, indoors: bool) -> bool {
    if indoors {
        let indoor_sqs = HashSet::from(sqs.iter()
                                          .filter(|sq| state.map[&sq].indoors())
                                          .collect::<HashSet<&(i32, i32, i8)>>());
        indoor_sqs.contains(&loc)
    } else {
        sqs.contains(&loc)
    }
}

pub fn pick_villager_name(used_names: &HashSet<String>) -> String {
    let names: [&str; 12] = ["Galleren", "Jaquette", "Aalis", "Martin", "Brida", "Cecillia",
        "Gotleib", "Ulrich", "Magda", "Sofiya", "Milivoj", "Velimer"];

    loop {
        let n = thread_rng().gen_range(0, names.len());
        if !used_names.contains(names[n]) {
            return String::from(names[n]);
        }
    }
}

impl GameObject for Villager {
    fn blocks(&self) -> bool {
        true
    }

    fn get_location(&self) -> (i32, i32, i8) {
        self.stats.location
    }

    fn set_location(&mut self, loc: (i32, i32, i8)) {
        self.stats.location = loc;
    }

    fn receive_event(&mut self, event: EventType, state: &mut GameState) -> Option<EventType> {
        None
    }

    fn get_fullname(&self) -> String {
        self.stats.name.clone()
    }

    fn get_object_id(&self) -> usize {
        self.object_id
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.stats.color, self.stats.ch)
    }

    fn as_item(&self) -> Option<Item> {
        None
    }

    fn as_npc(&self) -> Option<Box<dyn Actor>> {
        let npc = self.clone();
        Some(Box::new(npc))
    }
}