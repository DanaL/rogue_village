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

use super::{GameState, Map, NPCTable};

use crate::dialogue;
use crate::dialogue::DialogueLibrary;
use crate::display::{LIGHT_GREY};
use crate::map::{Tile, DoorState};
use crate::pathfinding::find_path;
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
    fn act(&mut self, state: &mut GameState, npcs: &mut NPCTable);
    fn get_tile(&self) -> Tile;
    fn get_loc(&self) -> (i32, i32, i8);
    fn get_name(&self) -> String;
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

pub struct Player {
	pub name: String,
	pub max_hp: u8,
	pub curr_hp: u8,
	pub location: (i32, i32, i8),
    pub vision_radius: u8,
}

impl Player {
    pub fn calc_vision_radius(&mut self, state: &mut GameState) {
        let prev_vr = self.vision_radius;
        let (hour, _) = state.curr_time();

        self.vision_radius = if hour >= 6 && hour <= 19 {
            99
        } else if hour >= 20 && hour <= 21 {
            8
        } else if hour >= 21 && hour <= 23 {
            7
        } else if hour < 4 {
            5
        } else if hour >= 4 && hour < 5 {
            7
        } else {
            9
        };

        // Announce sunrise and sunset if the player is on the surface
        if prev_vr == 99 && self.vision_radius == 9 && self.location.2 == 0 {
            state.write_msg_buff("The sun is beginning to set.");
        }
        if prev_vr == 5 && self.vision_radius == 7 && self.location.2 == 0 {
            state.write_msg_buff("Sunrise soon.");
        }
    }

    pub fn new(name: String) -> Player {
        let default_vision_radius = 99;

        Player {            
            name, max_hp: 10, curr_hp: 10, location: (0, 0, 0), vision_radius: default_vision_radius, 
        }
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Move((i32, i32, i8)),
    OpenDoor((i32, i32, i8)),
    CloseDoor((i32, i32, i8)),
}

#[derive(Clone, Debug)]
pub struct Mayor {
    pub stats: BasicStats,	
    pub facts_known: Vec<usize>,
    pub greeted_player: bool,
    pub home: HashSet<(i32, i32, i8)>,
    pub plan: VecDeque<Action>,
    pub voice: String,
    pub schedule: Vec<AgendaItem>,
}

impl Mayor {
    pub fn new(name: String, location: (i32, i32, i8), voice: &str) -> Mayor {
        let mut m = Mayor { stats: BasicStats::new(name, 8,  8, location,  '@',  LIGHT_GREY, Attitude::Stranger), 
            facts_known: Vec::new(), greeted_player: false, home: HashSet::new(),
            plan: VecDeque::new(), voice: String::from(voice), schedule: Vec::new(),
        };

        m.schedule.push(AgendaItem::new((9, 0), (21, 0), 0, Venue::TownSquare));
        m.schedule.push(AgendaItem::new((12, 0), (13, 0), 10    , Venue::Tavern));

        m
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

    fn try_to_move_to_loc(&mut self, loc: (i32, i32, i8), state: &mut GameState, npcs: &mut NPCTable) {
        if npcs.contains_key(&loc) || state.player_loc == loc {
            state.write_msg_buff("\"Excuse me.\"");
            self.plan.push_front(Action::Move(loc));
        } else if state.map[&loc] == Tile::Door(DoorState::Closed) {
            self.plan.push_front(Action::Move(loc));
            self.open_door(loc, state);
        } else {
            self.stats.location = loc;
        }
    }

    fn open_door(&mut self, loc: (i32, i32, i8), state: &mut GameState) {
        state.write_msg_buff("The mayor opens the door.");
        state.map.insert(loc, Tile::Door(DoorState::Open));
    }

    fn close_door(&mut self, loc: (i32, i32, i8), state: &mut GameState, npcs: &mut NPCTable) {
        if npcs.contains_key(&loc) || loc == state.player_loc {
            state.write_msg_buff("Please don't stand in the doorway.");
            self.plan.push_front(Action::CloseDoor(loc));
        } else {
            if let Tile::Door(DoorState::Open) = state.map[&loc] {
            state.write_msg_buff("The mayor closes the door.");
            state.map.insert(loc, Tile::Door(DoorState::Closed));
            }
        }
    }

    fn follow_plan(&mut self, state: &mut GameState, npcs: &mut NPCTable) {
        let action = self.plan.pop_front().unwrap();
        match action {
            Action::Move(loc) => self.try_to_move_to_loc(loc, state, npcs),
            Action::OpenDoor(loc) => self.open_door(loc, state),
            Action::CloseDoor(loc) => self.close_door(loc, state, npcs),
        }
    }

    fn entrance_location(&self, map: &Map) -> Option<(i32, i32, i8)> {
        for sq in &self.home {
            if let Tile::Door(_) = map[&sq] {
                return Some(*sq);
            }
        }

        None
    }

    fn is_home_open(&self, map: &Map) -> bool {
        match self.entrance_location(map) {
            Some(loc) => 
                if map[&loc] == Tile::Door(DoorState::Open) {
                    true
                } else {
                    false
                },
            _ => false
        }        
    }

    fn is_at_home(&self, map: &Map) -> bool {
        self.home.contains(&self.stats.location) 
                && map[&self.stats.location] != Tile::Door(DoorState::Open)
                && map[&self.stats.location] != Tile::Door(DoorState::Broken)
    }

    fn pick_spot_outside_home(&self, map: &Map) -> Option<(i32, i32, i8)> {
        let mut options = Vec::new();
        let entrance = self.entrance_location(map).unwrap();
        for adj in util::ADJ.iter() {
            let nl = (entrance.0 + adj.0, entrance.1 + adj.1, entrance.2);
            if !self.home.contains(&nl) && map[&nl].passable_dry_land() {
                options.push(nl);
            }
        }

        if options.len() > 0 {
            let j = thread_rng().gen_range(0, options.len());            
            Some(options[j])
        } else {
            None
        }        
    }

    fn set_day_schedule(&mut self, state: &GameState) {
        // During the day, mayor hangs around roughly in the town square.
        // When they leave their house in the morning, they'll want to close
        // their door.
        if self.is_at_home(&state.map) {
            match self.pick_spot_outside_home(&state.map) {
                Some(loc) => {
                    self.calc_plan_to_move(state, loc, false);
                    let entrance = self.entrance_location(&state.map).unwrap();
                    self.plan.push_back(Action::CloseDoor(entrance));
                },
                None => { /* This shouldn't happen... */ },
            }
        } else if !state.world_info.town_square.contains(&self.stats.location) {
            // Pick a random spot in the town square to move to
            let j = thread_rng().gen_range(0, state.world_info.town_square.len());
            let goal = state.world_info.town_square.iter().nth(j).unwrap();
            self.calc_plan_to_move(state, *goal, false);            
        } else {
            // otherwise just wander about the town square
            let j = thread_rng().gen_range(0, util::ADJ.len()) as usize;
            let d = util::ADJ[j];
            let adj = (self.stats.location.0 + d.0, self.stats.location.1 + d.1, self.stats.location.2);
            if state.world_info.town_square.contains(&adj) {
                self.calc_plan_to_move(state, adj, false);
            }
        }
    }

    fn set_evening_schedule(&mut self, state: &GameState) {
        // The evening plan is: the mayor wants to go home. Once home, they just
        // wander around in their house, although if their door is open, they close it.
        if !self.is_at_home(&state.map) {
            let j = thread_rng().gen_range(0, self.home.len());
            let goal_loc = self.home.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
            self.calc_plan_to_move(state, goal_loc, false);
        } else if self.is_home_open(&state.map) {
            let entrance = self.entrance_location(&state.map).unwrap();
            self.calc_plan_to_move(state, entrance, true);
            self.plan.push_back(Action::CloseDoor(entrance));
        } else {
            // for now, just wander about home
            let j = thread_rng().gen_range(0, self.home.len());
            let goal_loc = self.home.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
            if let Tile::Door(_) = state.map[&goal_loc] { }
            else {
                self.calc_plan_to_move(state, goal_loc, false); 
            }
        }
    }

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
                let b = &state.world_info.town_buildings.as_ref().unwrap().tavern;                
                if !in_location(state, self.get_loc(), &b, true) {
                    let j = thread_rng().gen_range(0, b.len());
                    let goal_loc = b.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
                    self.calc_plan_to_move(state, goal_loc, false);                
                } else {
                    self.idle_behaviour(state);
                }
            },
            Venue::TownSquare => {
                let ts = &state.world_info.town_square;
                if !in_location(state, self.get_loc(), ts, false) {
                    let j = thread_rng().gen_range(0, ts.len());
                    let goal_loc = ts.iter().nth(j).unwrap().clone(); // Clone prevents a compiler warning...
                    self.calc_plan_to_move(state, goal_loc, false);                    
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
            println!("Nothing on the agenda");
        } else {
            let item = &items[0].clone();
            self.check_agenda_item(state, item);
        }
    }
}

// Eventually I'll be able to reuse a bunch of this behaviour code for all Villagers
// (I hope) without cutting and pasting everywhere.
impl Actor for Mayor {
    fn act(&mut self, state: &mut GameState, npcs: &mut NPCTable) {
        // It's a mayoral duty to greet newcomers to town
        let pl = state.player_loc;
        
        if !self.greeted_player && pl.2 == self.stats.location.2 && util::distance(pl.0, pl.1, self.stats.location.0,self.stats.location.1) <= 4.0 {                 
            let s = format!("Hello stranger, welcome to {}!", state.world_info.town_name);
            state.write_msg_buff(&s);
            self.greeted_player = true;
        }

        if self.plan.len() > 0 {
            self.follow_plan(state, npcs);            
        } else {
            self.check_schedule(state);
        } 
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.stats.color, self.stats.ch)
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        self.stats.location
    }

    fn get_name(&self) -> String {
        String::from(&self.stats.name)
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
    fn act(&mut self, _state: &mut GameState, _npcs: &mut NPCTable) {
        
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.stats.color, self.stats.ch)
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        self.stats.location
    }
    
    fn get_name(&self) -> String {
        String::from(&self.stats.name)
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