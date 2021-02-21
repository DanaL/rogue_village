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

use std::collections::HashSet;

use rand::thread_rng;
use rand::Rng;

use super::{GameState, NPCTable};

use crate::display::{LIGHT_GREY, BRIGHT_RED};
use crate::map::Tile;
use crate::pathfinding::find_path;
use crate::util;

#[derive(Clone, Debug)]
pub enum Goal {
    Idle,
    GoTo((i32, i32, i8)),
}

pub trait Actor: ActorClone {
    fn act(&mut self, state: &mut GameState, npcs: &mut NPCTable);
    fn get_tile(&self) -> Tile;
}

pub trait ActorClone {
    fn clone_box(&self) -> Box<dyn Actor>;
}

impl<T> ActorClone for T 
where T: 'static + Actor + Clone,
{
    fn clone_box(&self) -> Box<dyn Actor> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Actor> {
    fn clone(&self) -> Box<dyn Actor> {
        self.clone_box()
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
    pub curr_path: Vec<(i32, i32)>,
}

impl Mayor {
    pub fn new(name: String, location: (i32, i32, i8)) -> Mayor {
        Mayor { name, max_hp: 8, curr_hp: 8, location, ch: '@', color: LIGHT_GREY, 
            facts_known: Vec::new(), greeted_player: false, home: HashSet::new(),
            goal: Goal::Idle, curr_path: Vec::new(),
        }
    }

    fn move_to(&mut self, state: &GameState, goal: (i32, i32, i8)) {
        if self.curr_path.len() == 0 {
            let mut passable = HashSet::new();
            passable.insert(Tile::Grass);
            passable.insert(Tile::Dirt);
            passable.insert(Tile::Tree);
            passable.insert(Tile::Door(true));
            passable.insert(Tile::Door(false));
            passable.insert(Tile::StoneFloor);
            passable.insert(Tile::Floor);

            let mut path = find_path(&state.map, self.location.0, self.location.1, self.location.2,
                goal.0, goal.1, 50, &passable);
            path.pop(); // first square in path is the start location
            println!("{:?}", path);
            self.curr_path = path;
        }

        let loc = &self.curr_path[0];
        self.location = (loc.0, loc.1, self.location.2);
        self.curr_path.pop();
    }
}

impl Actor for Mayor {
    fn act(&mut self, state: &mut GameState, npcs: &mut NPCTable) {
        // It's a mayoral duty to greet newcomers to town
        let pl = state.player_loc;
        if !self.greeted_player && pl.2 == self.location.2 && util::distance(pl.0, pl.1, self.location.0,self.location.1) <= 4 {     
            for j in 0..self.facts_known.len() {
                if state.world_info.facts[j].detail.starts_with("town name is ") {
                    let town_name = &state.world_info.facts[j].detail[13..];
                    let s = format!("Hello stranger, welcome to {}!", town_name);
                    state.write_msg_buff(&s);
                    self.greeted_player = true;
                }
            }
        }

        // Their schedule is: during 'business hours', hang out in the centre of the village. After
        // hours they want to hang out in their home. (Eventually of course there will also be the pub)
        // Gotta think of a good structure for schedules so that I don't have to hardcode all the rules
        // So: if between 9:00 and 21:00, mayor wants to be Idle near the town center. From 21:00 to 9:00
        // they want to be idle in their home
        if (state.curr_hour() >= 21 || state.curr_hour() <= 9) {
            if !self.home.contains(&self.location) {
                let j = thread_rng().gen_range(0, self.home.len());
                let goal_loc = self.home.iter().nth(j).unwrap();
                self.goal = Goal::GoTo(*goal_loc);
            } else {
                // hang out and be idle
                self.goal = Goal::Idle;
            }
        }
        else {
            let tb = state.world_info.town_boundary;
            //let town_centre = ((tb.0 + tb.2) / 2, (tb.1 + tb.3) / 2);
            let town_centre = (120, 79, 0);
            //println!("{} {}, {} {}", self.location.0, self.location.1, town_centre.0, town_centre.1);
            //println!("{}", util::distance(self.location.0, self.location.1, town_centre.0, town_centre.1));
            //println!("{:?}", state.map[&(town_centre.0, town_centre.1, self.location.2)]);
            if util::distance(self.location.0, self.location.1, town_centre.0, town_centre.1) > 4 {
                self.goal = Goal::GoTo((town_centre.0, town_centre.1, self.location.2));
            }
            else {
                self.goal = Goal::Idle;
            }
        }
        
        // Maybe create 'plans' for NPCs? So they have can a series of goals they want to 
        // accompalish?
        match self.goal {
            Goal::GoTo(loc) => {
                if self.location == loc {
                    self.goal = Goal::Idle; // We've reached our goal
                } else {
                    self.move_to(state, loc);
                }
            },
            Goal::Idle => { /* do nothing for moment */ },
        } 
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.color, self.ch)
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
}