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

use std::collections::{HashSet, VecDeque};

use rand::thread_rng;
use rand::Rng;
use std::time::{Duration, Instant};

use super::{GameState, NPCTable};

use crate::display::{LIGHT_GREY};
use crate::map::{Tile, DoorState};
use crate::pathfinding::find_path;
use crate::util;

#[derive(Clone, Debug)]
pub enum Goal {
    Idle,
    GoTo((i32, i32, i8)),
}
pub trait Actor {
    fn act(&mut self, state: &mut GameState, npcs: &mut NPCTable);
    fn get_tile(&self) -> Tile;
    fn get_loc(&self) -> (i32, i32, i8);
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
        let curr_time = state.curr_hour();

        self.vision_radius = if curr_time >= 6 && curr_time <= 19 {
            99
        } else if curr_time >= 20 && curr_time <= 21 {
            8
        } else if curr_time >= 21 && curr_time <= 23 {
            7
        } else if curr_time < 4 {
            5
        } else if curr_time >= 4 && curr_time < 5 {
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
    pub name: String,
	pub max_hp: u8,
	pub curr_hp: u8,
	pub location: (i32, i32, i8),
    pub ch: char,
    pub color: (u8, u8, u8),
    pub facts_known: Vec<usize>,
    pub greeted_player: bool,
    pub home: HashSet<(i32, i32, i8)>,
    pub goal: Goal,
    pub plan: VecDeque<Action>,
}

impl Mayor {
    pub fn new(name: String, location: (i32, i32, i8)) -> Mayor {
        Mayor { name, max_hp: 8, curr_hp: 8, location, ch: '@', color: LIGHT_GREY, 
            facts_known: Vec::new(), greeted_player: false, home: HashSet::new(),
            goal: Goal::Idle, plan: VecDeque::new(),
        }
    }

    fn calc_plan_to_move(&mut self, state: &GameState, goal: (i32, i32, i8), stop_before: bool) {
        if self.plan.len() == 0 {
            let mut passable = HashSet::new();
            passable.insert(Tile::Grass);
            passable.insert(Tile::Dirt);
            passable.insert(Tile::Tree);
            passable.insert(Tile::Door(DoorState::Open));
            passable.insert(Tile::Door(DoorState::Closed));
            passable.insert(Tile::Door(DoorState::Broken));
            passable.insert(Tile::Door(DoorState::Locked));
            passable.insert(Tile::StoneFloor);
            passable.insert(Tile::Floor);

            let mut path = find_path(&state.map, stop_before, self.location.0, self.location.1, 
                self.location.2, goal.0, goal.1, 50, &passable);
            
            path.pop(); // first square in path is the start location
            while path.len() > 0 {
                let sq = path.pop().unwrap();
                self.plan.push_back(Action::Move((sq.0, sq.1, self.location.2)));
            }
        }
    }

    // There's a bug here in that if the mayor's door was already open, they don't close it upon
    // entering their house because atm I'm only adding closing it to the plan after they open it.
    // What I should do is update their behaviour so that if they are in their house and the door is open
    // they'll try to close it. This probably means I need to add support for more than one Goal and
    // also currently the mayor doesn't know about the door to their house. They probably want to lock it
    // at night too.
    fn try_to_move_to_loc(&mut self, loc: (i32, i32, i8), state: &mut GameState, npcs: &mut NPCTable) {
        if npcs.contains_key(&loc) || state.player_loc == loc {
            state.write_msg_buff("\"Excuse me.\"");
            self.plan.push_front(Action::Move(loc));
        } else if state.map[&loc] == Tile::Door(DoorState::Closed) {
            let next = self.plan.pop_front().unwrap();
            self.plan.push_front(Action::Move(loc));
            self.open_door(loc, state);
        } else {
            self.location = loc;
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

    fn set_evening_schedule(&mut self, state: &mut GameState) {
        // We have to pick their new plan, if any. Their schedule is: during 'business hours', 
        // hang around the centre of the village. In the evening, they want to go home. If they
        // are home in the evening and the door is open, close it
        let is_evening = state.curr_hour() >= 21 || state.curr_hour() <= 9;
        if is_evening {
            let in_home = self.home.contains(&self.location) 
                && state.map[&self.location] != Tile::Door(DoorState::Open)
                && state.map[&self.location] != Tile::Door(DoorState::Broken);
            
            let mut entrance = (0, 0, 0);
            for sq in &self.home {
                if let Tile::Door(DoorState::Open) = state.map[&sq] {
                    entrance = *sq;
                    break;
                }                    
            }
                
            if !in_home {
                let j = thread_rng().gen_range(0, self.home.len());
                let goal_loc = self.home.iter().nth(j).unwrap();
                self.calc_plan_to_move(state, *goal_loc, false);
            } else if state.map[&entrance] == Tile::Door(DoorState::Open) {
                self.calc_plan_to_move(state, entrance, true);
                self.plan.push_back(Action::CloseDoor(entrance));
            } else {
                // for now, just wander about home
                let j = thread_rng().gen_range(0, self.home.len());
                let goal_loc = self.home.iter().nth(j).unwrap();
                if let Tile::Door(_) = state.map[&goal_loc] { }
                else {
                    self.calc_plan_to_move(state, *goal_loc, false);
                }
            }
        }
    }
}

// Eventually I'll be able to reuse a bunch of this behaviour code for all Villagers
// (I hope) without cutting and pasting everywhere.
impl Actor for Mayor {
    fn act(&mut self, state: &mut GameState, npcs: &mut NPCTable) {
        // It's a mayoral duty to greet newcomers to town
        let pl = state.player_loc;
        if !self.greeted_player && pl.2 == self.location.2 && util::distance(pl.0, pl.1, self.location.0,self.location.1) <= 4.0 {     
            for j in 0..self.facts_known.len() {
                if state.world_info.facts[j].detail.starts_with("town name is ") {
                    let town_name = &state.world_info.facts[j].detail[13..];
                    let s = format!("Hello stranger, welcome to {}!", town_name);
                    state.write_msg_buff(&s);
                    self.greeted_player = true;
                }
            }
        }

        if self.plan.len() > 0 {
            self.follow_plan(state, npcs);
            return;
        } else {
            self.set_evening_schedule(state);
        }

        // Their schedule is: during 'business hours', hang out in the centre of the village. After
        // hours they want to hang out in their home. (Eventually of course there will also be the pub)
        // Gotta think of a good structure for schedules so that I don't have to hardcode all the rules
        // So: if between 9:00 and 21:00, mayor wants to be Idle near the town center. From 21:00 to 9:00
        // // they want to be idle in their home
        // else if state.curr_hour() >= 21 || state.curr_hour() <= 9 {
        //     let in_home = self.home.contains(&self.location) 
        //         && state.map[&self.location] != Tile::Door(DoorState::Open)
        //         && state.map[&self.location] != Tile::Door(DoorState::Broken);

        //     if !in_home {
        //             && state.map[&self.location] != Tile::Door(DoorState::Open)
        //             && state.map[&self.location] != Tile::Door(DoorState::Broken) {
        //         let j = thread_rng().gen_range(0, self.home.len());
        //         let goal_loc = self.home.iter().nth(j).unwrap();
        //         self.calc_plan_to_move(state, goal_loc);
        //     } else {
        //         // hang out and be idle
        //         self.goal = Goal::Idle;
        //     }
        // } else {
        //     let tb = state.world_info.town_boundary;
        //     //let town_centre = ((tb.0 + tb.2) / 2, (tb.1 + tb.3) / 2);
        //     let town_centre = (120, 79, 0);
        //     //println!("{} {}, {} {}", self.location.0, self.location.1, town_centre.0, town_centre.1);
        //     //println!("{}", util::distance(self.location.0, self.location.1, town_centre.0, town_centre.1));
        //     //println!("{:?}", state.map[&(town_centre.0, town_centre.1, self.location.2)]);
            
        //     if util::distance(self.location.0, self.location.1, town_centre.0, town_centre.1) > 4.0 {
        //         self.goal = Goal::GoTo((town_centre.0, town_centre.1, self.location.2));
        //     }
        //     else {
        //         self.goal = Goal::Idle;
        //     }            
        // }
        
        // Maybe create 'plans' for NPCs? So they have can a series of goals they want to 
        // accompalish?
        // match self.goal {
        //     Goal::GoTo(loc) => {
        //         if self.location == loc {
        //             self.goal = Goal::Idle; // We've reached our goal
        //         } else {
        //             //let pf_start = Instant::now();
        //             self.calc_plan_to_move(state, loc);
        //             //let pf_duration = pf_start.elapsed();
        //             //println!("Time for pf: {:?}", pf_duration);
        //         }
        //     },
        //     Goal::Idle => { /* do nothing for moment */ },
        // } 
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.color, self.ch)
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        self.location
    }
}

#[derive(Clone, Debug)]
pub struct SimpleMonster {
    pub name: String,
    pub max_hp: u8,
    pub curr_hp: u8,
    pub location: (i32, i32, i8),
    pub ch: char,
    pub color: (u8, u8, u8),
    pub goal: Goal,
}

impl SimpleMonster {
    pub fn new(name: String, location:( i32, i32, i8), ch: char, color: (u8, u8, u8)) -> SimpleMonster {
        SimpleMonster { name, max_hp: 8, curr_hp: 8, location, ch, color, goal: Goal::Idle }
    }
}

impl Actor for SimpleMonster {
    fn act(&mut self, state: &mut GameState, npcs: &mut NPCTable) {
        //let s = format!("The {} looks around for prey!", self.name);
        //println!("{}", s);
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.color, self.ch)
    }

    fn get_loc(&self) -> (i32, i32, i8) {
        self.location
    }
}