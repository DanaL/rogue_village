// This file is part of RogueVillage, a roguelike game.
//
// RogueVillage is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// YarrL is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with RogueVillage.  If not, see <https://www.gnu.org/licenses/>.

use super::{GameState};

use crate::display::{LIGHT_GREY, BRIGHT_RED};
use crate::map::Tile;
use crate::util;

pub trait Actor {
    fn act(&mut self, state: &mut GameState);
    fn get_tile(&self) -> Tile;
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
        let curr_time = (state.turn / 100 + 12) % 24;

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

pub struct Mayor {
    pub name: String,
	pub max_hp: u8,
	pub curr_hp: u8,
	pub location: (i32, i32, i8),
    pub ch: char,
    pub color: (u8, u8, u8),
    pub facts_known: Vec<usize>,
    pub greeted_player: bool,
}

impl Mayor {
    pub fn new(name: String, location: (i32, i32, i8)) -> Mayor {
        Mayor { name, max_hp: 8, curr_hp: 8, location, ch: '@', color: LIGHT_GREY, 
            facts_known: Vec::new(), greeted_player: false }
    }
}

impl Actor for Mayor {
    fn act(&mut self, state: &mut GameState) {
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

        let tb = state.world_info.town_boundary;
        let town_centre = ((tb.0 + tb.2) / 2, (tb.1 + tb.3) / 2);
        // During business hours, the mayor will just wander around but not too far from town centre. If they are
        // too far from town centre, move back towards it
        if util::distance(self.location.0, self.location.1, town_centre.0, town_centre.1) > 4 {
            
        } else {
            
        }        
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.color, self.ch)
    }
}

pub struct SimpleMonster {
    pub name: String,
    pub max_hp: u8,
    pub curr_hp: u8,
    pub location: (i32, i32, i8),
    pub ch: char,
    pub color: (u8, u8, u8),
}

impl SimpleMonster {
    pub fn new(name: String, location:( i32, i32, i8), ch: char, color: (u8, u8, u8)) -> SimpleMonster {
        SimpleMonster { name, max_hp: 8, curr_hp: 8, location, ch, color }
    }
}

impl Actor for SimpleMonster {
    fn act(&mut self, state: &mut GameState) {
        //let s = format!("The {} looks around for prey!", self.name);
        //println!("{}", s);
    }

    fn get_tile(&self) -> Tile {
        Tile::Creature(self.color, self.ch)
    }
}