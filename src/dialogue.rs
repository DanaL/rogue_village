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

use std::{collections::HashMap};
use std::fs;

use rand::{Rng, thread_rng};

use super::GameState;
use crate::actor::Attitude;
use crate::util::StringUtils;

pub type DialogueLibrary = HashMap<String, HashMap<Attitude, Vec<String>>>;

fn init_lines_hashmap() -> HashMap<Attitude, Vec<String>> {
    let mut lines = HashMap::new();
    lines.insert(Attitude::Stranger, Vec::new());
    lines.insert(Attitude::Indifferent, Vec::new());
    lines.insert(Attitude::Friendly, Vec::new());
    lines.insert(Attitude::Hostile, Vec::new());

    lines
}

// Not doing any error checking yet; assuming a well-formed dialogue file...
pub fn read_dialogue_lib() -> DialogueLibrary {
    let mut dl: DialogueLibrary = HashMap::new();

    let contents = fs::read_to_string("dialogue.txt")
        .expect("Unable to find dialogue file!");
    
    let mut curr_voice = "";
    let mut curr_attitude = Attitude::Stranger;
    for line in contents.split('\n') {        
        if line.starts_with("voice:") {
            let pieces: Vec<&str> = line.split(':').collect();
            curr_voice = pieces[1];
            dl.insert(String::from(curr_voice), init_lines_hashmap());
        } else if line.starts_with("-") {
            let v = dl.get_mut(curr_voice).unwrap();
            let a = v.get_mut(&curr_attitude).unwrap();
            a.push(line[2..].to_string());
        } else if line.starts_with('#') {
            continue;   
        } else {
            curr_attitude = match line {
                "Indifferent" => Attitude::Indifferent,
                "Friendly" => Attitude::Friendly,
                "Hostile" => Attitude::Hostile,
                _ => Attitude::Stranger,
            };
        }
    }

    dl
}

pub fn pick_voice_line(lib: &DialogueLibrary, voice: &str, attitude: Attitude) -> String {
    let j = thread_rng().gen_range(0, lib[voice][&attitude].len());
    
    String::from(&lib[voice][&attitude][j])
}

pub fn calc_direction(start: (i32, i32, i8), dest: (i32, i32, i8)) -> String {
    let x = (dest.0 - start.0) as f64;
    let y = (dest.1 - start.1) as f64;
    let angle = f64::atan2(x, y);

    // I feel like there is some trig or conversion to make this less gross...    
    if f64::abs(angle) < 0.236 {
        "east".to_string()
    } else if f64::abs(angle) > 2.904 {
        "west".to_string()
    } else if angle <= -0.236 && angle >= -1.334 {
        "northeast".to_string()
    } else if angle < -1.334 && angle >= -1.81 {
        "north".to_string()
    } else if angle < -1.81 && angle >= -2.904 {
        "northwest".to_string()
    } else if angle >= 0.236 && angle <= 1.334 {
        "southeast".to_string()
    } else if angle > 1.334 && angle <= 1.81 {
        "south".to_string()
    } else {
        "southwest".to_string()
    }
}

pub fn parse_voice_line(line: &str, state: &GameState, speaker: &str, speaker_loc: (i32, i32, i8), extra_info: Option<HashMap<String, String>>) -> String {
    // this is a dead stupid implementation but at the moment my dialogue lines are pretty simple
    let mut s = line.replace("{village}", &state.world_info.town_name);
    s = s.replace("{player-name}", &state.world_info.player_name);
    s = s.replace("{name}", speaker);
    s = s.replace("{inn-name}", &state.world_info.tavern_name);

    if line.contains("{dungeon-dir}") {
        for fact in &state.world_info.facts {
            if fact.detail == "dungeon location" {
                let dir = calc_direction(speaker_loc, fact.location);
                s = s.replace("{dungeon-dir}", &dir);
                break;
            }
        }        
    }

    if line.contains("{time-greeting}") {
        let time = state.curr_time();
        if time.0 >= 6 && time.0 < 12 {
            s = s.replace("{time-greeting}", "good morning");
        } else if time.0 >= 12 && time.0 < 21 {
            s = s.replace("{time-greeting}", "good evening");
        } else {
            s = s.replace("{time-greeting}", "*yawn*");
        }
    }

    if let Some(extra_info) = extra_info {
        let keys: Vec<String> = extra_info.keys().map(|s| s.to_string()).collect();

        for key in keys {
            if line.contains(&key) {
                s = s.replace(&key, &extra_info[&key]);
            }
        }
    }

    s.capitalize()
}

pub fn rnd_innkeeper_voice() -> String {
    let contents = fs::read_to_string("dialogue.txt")
        .expect("Unable to find dialogue file!");
    
    let mut voices = Vec::new();
    for line in contents.split('\n') {        
        if line.starts_with("voice:innkeeper") {
            let pieces: Vec<&str> = line.split(':').collect();
            voices.push(pieces[1]);
        }
    }

    let mut rng = rand::thread_rng();
    let pick = rng.gen_range(0, voices.len());

    voices[pick].to_string()
}