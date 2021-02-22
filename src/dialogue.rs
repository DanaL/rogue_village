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

use std::{collections::HashMap, ops::Index};
use std::fs;

use crate::actor::{Attitude};

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