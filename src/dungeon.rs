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

use rand::thread_rng;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::map::{Tile, DoorState};
use crate::util;
#[derive(Debug)]
pub struct Vault {
    r1: i32,
    c1: i32,
    r2: i32,
    c2: i32,
    entrance: (i32, i32),
}

impl Vault {
    pub fn new(r1: i32, c1: i32, r2: i32, c2: i32, entrance: (i32, i32)) -> Vault {
        Vault { r1, c1, r2, c2, entrance }
    }
}

fn pick_room() -> (Vec<Vec<Tile>>, usize, usize) {
    let mut rng = rand::thread_rng();
    let rn = rng.gen_range(0.0, 1.0);
    let mut height;
    let mut width;
    let mut room: Vec<Vec<Tile>> = Vec::new();
    if rn < 0.8 {
        // make a rectangular room
        height = rng.gen_range(5, 9);
        width = rng.gen_range(5, 26);
        let row = vec![Tile::Wall; width + 2];
        room.push(row);
        for _ in 0..height {
            let mut row = Vec::new();
            row.push(Tile::Wall);
            for _ in 0..width {
                row.push(Tile::StoneFloor);
            }
            row.push(Tile::Wall);
            room.push(row);
        }
        let row = vec![Tile::Wall; width + 2];
        room.push(row);

        height += 2;
        width += 2;
    } else {
        // make a circular room
        let radius = rng.gen_range(3, 7);
        for _ in 0..radius*2 + 3 {
            room.push(vec![Tile::Wall; radius * 2 + 3]);
        }
        height = radius * 2 + 3;
        width = radius * 2 + 3;
        let mut x = radius;
        let mut y = 0;
        let mut error: i32 = 0;
        let mut sqrx_inc: i32 = 2 * radius as i32 - 1;
        let mut sqry_inc: i32 = 1;
        let rc = radius + 1;
        let cc = radius + 1;

        // This draws the outline of a cricle via Bresenham
        while y <= x {
            room[rc + y][cc + x] = Tile::StoneFloor;
            room[rc + y][cc - x] = Tile::StoneFloor;
            room[rc - y][cc + x] = Tile::StoneFloor;
            room[rc - y][cc - x] = Tile::StoneFloor;
            room[rc + x][cc + y] = Tile::StoneFloor;
            room[rc + x][cc - y] = Tile::StoneFloor;
            room[rc - x][cc + y] = Tile::StoneFloor;
            room[rc - x][cc - y] = Tile::StoneFloor;

            y += 1;
            error += sqry_inc;
            sqry_inc += 2;
            if error > x as i32 {
                x -= 1;
                error -= sqrx_inc;
                sqrx_inc -= 2;
            }
        }

        // Now turn all the squares inside the circle into floors
        for r in 1..height - 1 {
            for c in 1..width - 1 {
                if util::distance(r as i32, c as i32, rc as i32, cc as i32) as usize <= radius {
                    room[r][c] = Tile::StoneFloor;
                }                
            }
        }
    }

    (room, height, width)
}

fn copy_room(room: &Vec<Vec<Tile>>) -> Vec<Vec<Tile>> {
    let mut room2 = Vec::new();
    for row in room {
        let mut row2 = Vec::new();
        for t in row {
            row2.push(*t);
        }
        room2.push(row2);
    }

    room2
}

fn draw_room(level: &mut Vec<Tile>, row: usize, col: usize, room: &Vec<Vec<Tile>>, width: usize) {
    let mut curr_row = row;
    for line in room {
        for curr_col in 0..line.len() {
            level[curr_row * width + col + curr_col] = line[curr_col];
        }
        curr_row += 1;
    }
}

// (start_row, end_row, start_col, end_col)
fn room_fits(level: &mut Vec<Tile>, bounds: (i32, i32, i32, i32), width: usize) -> bool {    
    if bounds.0 <= 0 {
        return false;
    } else if bounds.1 as usize >= level.len() / width - 1 {
        return false;
    } else if bounds.2 <= 1 {
        return false;
    } else if bounds.3 as usize >= width - 1 {
        return false;
    }

    for r in bounds.0 as usize..bounds.1 as usize {
        for c in bounds.2 as usize..bounds.3 as usize {
            if level[r * width + c] != Tile::Wall {
                return false
            }
        }
    }

    true
}

fn add_doorway_horizonal(level: &mut Vec<Tile>, row: usize, lo: usize, hi: usize, width: usize) {
    let mut rng = thread_rng();
    let mut options = Vec::new();
    for col in lo..hi {
        if level[(row - 1) * width + col] == Tile::StoneFloor && level[(row + 1) * width + col] == Tile::StoneFloor {
            options.push(col);
        }
    }

    if options.len() > 0 {        
        let x = rng.gen_range(0, options.len());
        let col = options[x];
        if rng.gen_range(0.0, 1.0) < 0.8 {
            // Have to make them locked sometimes too...
            level[row * width + col] = Tile::Door(DoorState::Closed);
        } else {
            level[row * width + col] = Tile::StoneFloor;
        }
    } else {
        // if there are no options for a 1-thickness wall to place an entranceway, make a short
        // hallway between the two rooms.
        let col = (lo + hi) / 2;
        let mut row2 = row;
        while level[row2 * width + col] != Tile::StoneFloor {
            level[row2 * width + col] = Tile::StoneFloor;
            row2 -= 1;
        }
        let mut row2 = row + 1;
        while level[row2 * width + col] != Tile::StoneFloor {
            level[row2 * width + col] = Tile::StoneFloor;
            row2 += 1;
        }
    }
}

fn add_doorway_vertical(level: &mut Vec<Tile>, col: usize, lo: usize, hi: usize, width: usize) {    
    let mut rng = thread_rng();
    let mut options = Vec::new();
    for row in lo..hi {
        if level[row * width + col - 1] == Tile::StoneFloor && level[row * width + col + 1] == Tile::StoneFloor {
            options.push(row);
        }
    }

    if options.len() > 0 {        
        let x = rng.gen_range(0, options.len());
        let row = options[x];
        if rng.gen_range(0.0, 1.0) < 0.8 {
            level[row * width + col] = Tile::Door(DoorState::Closed);
        } else {
            level[row * width + col] = Tile::StoneFloor;
        }
    } else {
        // if there are no options for a 1-thickness wall to place an entranceway, make a short
        // hallway between the two rooms.        
        let row = (lo + hi) / 2;
        let mut col2 = col;
        
        while level[row * width + col2] != Tile::StoneFloor {
            level[row * width + col2] = Tile::StoneFloor;
            col2 -= 1;
        }
        let mut col2 = col + 1;
        while level[row * width + col2] != Tile::StoneFloor {
            level[row * width + col2] = Tile::StoneFloor;
            col2 += 1;
        }
    }
}

fn place_room(level: &mut Vec<Tile>, rooms: &mut Vec<(Vec<Vec<Tile>>, usize, usize, usize, usize, &str)>,
    parent_index: usize, room: &(Vec<Vec<Tile>>, usize, usize), width: usize) -> bool {

    let mut rng = thread_rng();
    let mut sides = vec!['n', 's', 'e', 'w'];
    sides.shuffle(&mut rng);

    // We'll try a few times per side to place the new room
    let num_of_tries = 5;
    while sides.len() > 0 {
        let side = sides.pop().unwrap();        
        if side == 'n' {
            for _ in 0..num_of_tries {
                let end_row: i32 = rooms[parent_index].1 as i32 + 1;
                let start_col: i32 = rng.gen_range(rooms[parent_index].2 + 1, rooms[parent_index].4 - 5) as i32;
                let start_row: i32 = end_row - room.1 as i32;
                let end_col: i32 = start_col + room.2 as i32;
                let bounds = (start_row, end_row, start_col, end_col);
                if room_fits(level, bounds, width) {
                    let new_room = copy_room(&room.0);
                    draw_room(level, start_row as usize, start_col as usize, &new_room, width);
                    rooms.push((new_room, start_row as usize, start_col as usize, end_row as usize, end_col as usize, "N"));
                    
                    let lo = if rooms[parent_index].2 + 1 > start_col as usize {
                        rooms[parent_index].2 + 1
                    } else {
                        start_col as usize
                    };
                    let hi = if rooms[parent_index].4 - 1 < end_col as usize {
                        rooms[parent_index].4 - 1
                    } else {
                        end_col as usize
                    };
                    add_doorway_horizonal(level, end_row as usize - 1, lo as usize, hi as usize, width);
                    return true;
                }
            }
        } else if side == 's' {
            for _ in 0..num_of_tries {
                let start_row: i32 = rooms[parent_index].3 as i32 - 1;
                let start_col: i32 = rng.gen_range(rooms[parent_index].2 + 1, rooms[parent_index].4 - 5) as i32;
                let end_row: i32 = start_row + room.1 as i32;
                let end_col: i32 = start_col + room.2 as i32;
                let bounds = (start_row, end_row, start_col, end_col);
                if room_fits(level, bounds, width) {
                    let new_room = copy_room(&room.0);
                    draw_room(level, start_row as usize, start_col as usize, &new_room, width);
                    rooms.push((new_room, start_row as usize, start_col as usize, end_row as usize, end_col as usize, "S"));
                    
                    let lo = if rooms[parent_index].2 + 1 > start_col as usize {
                        rooms[parent_index].2 + 1
                    } else {
                        start_col as usize
                    };
                    let hi = if rooms[parent_index].4 - 1 < end_col as usize {
                        rooms[parent_index].4 - 1
                    } else {
                        end_col as usize
                    };
                    add_doorway_horizonal(level, start_row as usize, lo as usize, hi as usize, width);
                    return true;
                }
            }
        } else if side == 'w' {
            for _ in 0..num_of_tries {
                let end_col = rooms[parent_index].2 as i32 + 1;
                let start_row = rng.gen_range(rooms[parent_index].1 + 1, rooms[parent_index].3 - 5) as i32;
                let start_col = end_col - room.2 as i32;
                let end_row = start_row + room.1 as i32;
                let bounds = (start_row, end_row, start_col, end_col);
                if room_fits(level, bounds, width) {
                    let new_room = copy_room(&room.0);
                    draw_room(level, start_row as usize, start_col as usize, &new_room, width);
                    rooms.push((new_room, start_row as usize, start_col as usize, end_row as usize, end_col as usize, "W"));
                    let lo = if rooms[parent_index].1 + 1 > start_row as usize {
                        rooms[parent_index].1 + 1
                    } else {
                        start_row as usize
                    };
                    let hi = if rooms[parent_index].3 - 1 < end_row as usize {
                        rooms[parent_index].3 - 1
                    } else {
                        end_row as usize
                    };
                    add_doorway_vertical(level, end_col as usize - 1, lo as usize, hi as usize, width);
                    return true;
                }
            }
        } else if side == 'e' {
            for _ in 0..num_of_tries {
                let start_col = rooms[parent_index].4 as i32 - 1;
                let start_row = rng.gen_range(rooms[parent_index].1 + 1, rooms[parent_index].3 - 5) as i32;
                let end_col = start_col + room.2 as i32;
                let end_row = start_row + room.1 as i32;
                let bounds = (start_row, end_row, start_col, end_col);
                if room_fits(level, bounds, width) {
                    let new_room = copy_room(&room.0);
                    draw_room(level, start_row as usize, start_col as usize, &new_room, width);
                    rooms.push((new_room, start_row as usize, start_col as usize, end_row as usize, end_col as usize, "E"));
                    let lo = if rooms[parent_index].1 + 1 > start_row as usize {
                        rooms[parent_index].1 + 1
                    } else {
                        start_row as usize
                    };
                    let hi = if rooms[parent_index].3 - 1 < end_row as usize {
                        rooms[parent_index].3 - 1
                    } else {
                        end_row as usize
                    };
                    add_doorway_vertical(level, start_col as usize, lo as usize, hi as usize, width);
                    return true;
                }
            }
        }
    }
    
    false
}

fn find_spot_for_room(level: &mut Vec<Tile>, rooms: &mut Vec<(Vec<Vec<Tile>>, usize, usize, usize, usize, &str)>,
                            room: &(Vec<Vec<Tile>>, usize, usize), width: usize) -> bool {
    // We want to try every room in the dungeon so far to see if we can attach the new room to it
    let mut rng = thread_rng();
    let mut tries: Vec<usize> = (0..rooms.len()).collect();
    tries.shuffle(&mut rng);
    
    while tries.len() > 0 {
        let i = tries.pop().unwrap();
        if place_room(level, rooms, i, room, width) {
            return true;
        }
    }

    false
}

fn add_extra_door_to_horizontal_wall(level: &mut Vec<Tile>, width: usize, row: usize, col_lo: usize, col_hi: usize) -> bool {
    let mut rng = rand::thread_rng();
    let mut already_connected = false;
    let mut options = Vec::new();
    for col in col_lo..col_hi {
        if level[row * width + col] != Tile::Wall {
            already_connected = true;
            break;
        }

        if (row + 1) * width + col >= level.len() {
            println!("Hmm this shouldn't happen {} {} {}", row, col, (row + 1) * width + col);
        }
        if level[(row - 1) * width + col] == Tile::StoneFloor && level[(row + 1) * width + col] == Tile::StoneFloor {
            options.push(col);
        }
    }
    if !already_connected && options.len() > 0 {
        let x = rng.gen_range(0, options.len());
        let col = options[x];
        level[row * width + col] = Tile::Door(DoorState::Closed);
        return true;
    }

    false
}

fn add_extra_door_to_vertical_wall(level: &mut Vec<Tile>, width: usize, col: usize, row_lo: usize, row_hi: usize) -> bool {
    let mut rng = rand::thread_rng();
    let mut already_connected = false;
    let mut options = Vec::new();
    for row in row_lo..row_hi {
        if level[row * width + col] != Tile::Wall {
            already_connected = true;
            break;
        }
        if level[row * width + col - 1] == Tile::StoneFloor && level[row * width + col + 1] == Tile::StoneFloor {
            options.push(row);
        }
    }
    if !already_connected && options.len() > 0 {
        let x = rng.gen_range(0, options.len());
        let row = options[x];
        level[row * width + col] = Tile::Door(DoorState::Closed);
        return true;
    }

    false
}

// The first pass of placing rooms and connecting each new one by an entrance
// yields a map that has only a single path through it, ie acyclic. It's more 
// interesting to explore a dungeon with some loops. So this function finds places
// we can add doors between rooms that aren't currently connected.
// (These are probably also good candidates for secret doors once I implement those!)
fn add_extra_doors(level: &mut Vec<Tile>, rooms: &Vec<(Vec<Vec<Tile>>, usize, usize, usize, usize, &str)>, width: usize) {    
    let height = level.len() / width;

    for room in rooms {
        // check north wall
        if add_extra_door_to_horizontal_wall(level, width, room.1, room.2 + 1,room.4 - 1)  {
            continue;
        }        
        if (room.3 as usize) < height - 2 && add_extra_door_to_horizontal_wall(level, width, room.3 - 1, room.2 + 1,room.4 - 1)  {
            continue;
        }
        // check west wall
        if add_extra_door_to_vertical_wall(level, width, room.2, room.1 + 1, room.3 - 1) {
            continue;
        }
        // check east wall
        if add_extra_door_to_vertical_wall(level, width, room.4 - 1, room.1 + 1, room.3 - 1) {
            continue;
        }
    }
}

fn draw_corridor_north(level: &mut Vec<Tile>, row: usize, col: usize, width: usize) -> bool {
    let mut pts = vec![row * width + col];
    let mut dr = row;
    loop {
        dr -= 1;        
        if dr < 2 {
            return false;
        }
        let i = dr * width + col;
        if level[i - 1] != Tile::Wall || level[i + 1] != Tile::Wall {
            return false;
        }
        pts.push(i);
        if level[(dr - 1) * width + col] == Tile::StoneFloor {
            break;
        }
    }

    for i in pts {
        level[i] = Tile::StoneFloor;
    }

    true
}

fn draw_corridor_south(level: &mut Vec<Tile>, row: usize, col: usize, width: usize) -> bool {
    let mut pts = vec![row * width + col];
    let mut dr = row;
    let height = level.len() / width;
    loop {
        dr += 1;        
        if dr > height - 2 {
            return false;
        }
        let i = dr * width + col;
        if level[i - 1] != Tile::Wall || level[i + 1] != Tile::Wall {
            return false;
        }
        pts.push(i);
        if level[(dr + 1) * width + col] == Tile::StoneFloor {
            break;
        }
    }

    for i in pts {
        level[i] = Tile::StoneFloor;
    }

    true
}

fn draw_corridor_west(level: &mut Vec<Tile>, row: usize, col: usize, width: usize) -> bool {
    let mut pts = vec![row * width + col];
    let mut dc = col;
    loop {
        dc -= 1;
        if dc < 3 {
            return false;
        }
        let i = row * width + dc;
        if level[i - width] != Tile::Wall || level[i + width] != Tile::Wall {
            return false;
        }
        pts.push(i);
        if level[i - 1] == Tile::StoneFloor {
            break;
        }
    }

    for i in pts {
        level[i] = Tile::StoneFloor;
    }

    true
}

fn draw_corridor_east(level: &mut Vec<Tile>, row: usize, col: usize, width: usize) -> bool {
    let mut pts = vec![row * width + col];
    let mut dc = col;
    loop {
        dc += 1;
        if dc >= width - 3 {
            return false;
        }
        let i = row * width + dc;
        if level[i - width] != Tile::Wall || level[i + width] != Tile::Wall {
            return false;
        }
        pts.push(i);
        if level[i + 1] == Tile::StoneFloor {
            break;
        }
    }

    for i in pts {
        level[i] = Tile::StoneFloor;
    }
    
    true
}

// Once again, we'll look for walls that don't already have an egress
fn try_to_add_corridor(level: &mut Vec<Tile>, rooms: &Vec<(Vec<Vec<Tile>>, usize, usize, usize, usize, &str)>, width: usize) {
    let mut rng = rand::thread_rng();
    for room in rooms {
        // check east wall
        let col = room.4 - 1;
        let mut options = Vec::new();
        let mut already_connected = false;
        for row in room.1 + 1..room.3 - 1 {
            if level[row * width + col - 1] != Tile::StoneFloor {
                continue;
            }
            if level[row * width + col] != Tile::Wall {
                already_connected = true;                                
                break;
            }
            options.push(row);            
        }
        if !already_connected && options.len() > 0 {
            let x = rng.gen_range(0, options.len());
            let row = options[x];
            if draw_corridor_east(level, row, col, width) {
                return;
            }
        }
        
        // check west wall
        let col = room.2;
        let mut options = Vec::new();
        let mut already_connected = false;
        for row in room.1 + 1..room.3 - 1 {
            if level[row * width + col + 1] != Tile::StoneFloor {
                continue;
            }
            if level[row * width + col] != Tile::Wall {
                already_connected = true;                                
                break;
            }
            options.push(row);
        }
        if !already_connected && options.len() > 0 {
            let x = rng.gen_range(0, options.len());
            let row = options[x];
            if draw_corridor_west(level, row, col, width) {
                return;
            }
        }

        // check north wall
        let row = room.1;
        let mut options = Vec::new();
        let mut already_connected = false;
        for col in room.2 + 1..room.4 - 1 {
            if level[(row + 1) * width + col] != Tile::StoneFloor {
                continue;
            }
            if level[row * width + col] != Tile::Wall {
                already_connected = true;
                break;
            }
            options.push(col);
        }
        if !already_connected && options.len() > 0 {
            let x = rng.gen_range(0, options.len());
            let col = options[x];            
            if draw_corridor_north(level, row, col, width) {
                 return;
            }
        }

        // check south wall
        let row = room.3 - 1;
        let mut options = Vec::new();
        let mut already_connected = false;
        for col in room.2 + 1..room.4 - 1 {
            if level[(row - 1) * width + col] != Tile::StoneFloor {
                continue;
            }
            if level[row * width + col] != Tile::Wall {
                already_connected = true;
                break;
            }
            options.push(col);
        }
        if !already_connected && options.len() > 0 {
            let x = rng.gen_range(0, options.len());
            let col = options[x];            
            if draw_corridor_south(level, row, col, width) {
                 return;
            }
        }
    }
}

fn find_vaults(level: &Vec<Tile>,  width: usize, rooms: Vec<(usize, usize, usize, usize)>) -> Vec<Vault> {
    let mut vaults = Vec::new();

    for room in rooms.iter() {        
        let mut egresses = Vec::new();
        for col in room.1..room.3 {
            if level[room.0 * width + col] != Tile::Wall {
                egresses.push((room.0, col));                
            }
            if level[(room.2 - 1) * width + col] != Tile::Wall {
                egresses.push((room.2 - 1, col));
            }
        }
        for row in room.0..room.2 {
            if level[row * width + room.1] != Tile::Wall {
                egresses.push((row, room.1));
            }
            if level[row * width + room.3 - 1] != Tile::Wall {
                egresses.push((row, room.3 - 1));
            }
        }

        if egresses.len() == 1 {
            let entrance = (egresses[0].0 as i32, egresses[0].1 as i32);
            vaults.push(Vault::new(room.0 as i32, room.1 as i32, room.2 as i32, room.3 as i32, entrance))
        }
    }

    vaults
}

fn carve(level: &mut Vec<Tile>, width: usize, height: usize) -> Vec<Vault> {
    let mut rooms = Vec::new();
    let mut rng = rand::thread_rng();
    let center_row = (height / 2) as i16;
    let center_col = (width / 2) as i16;
    let row = (center_row + rng.gen_range(-6, 6)) as usize;
    let col = (center_col + rng.gen_range(-10, 10)) as usize;

    // Draw the starting room to the dungeon map. (This is just the first room we make on the
    // level, not necessaily the entrance room)
    let room = pick_room();
    draw_room(level, row as usize, col as usize, &room.0, width as usize);
    rooms.push((room.0, row, col, row + room.1, col + room.2, "Start"));

    loop {
        let room = pick_room();
        // keep trying to add new rooms until we fail to place one and that's probably
        // enough rooms for a decent dungeon level
        if !find_spot_for_room(level, &mut rooms, &room, width as usize) {
            break;
        }
    }

    add_extra_doors(level, &rooms, width as usize);

    // try to add up to three extra corridors between rooms
    for _ in 0..3 {
        try_to_add_corridor(level, &rooms, width as usize);
    }

    let room_borders: Vec<(usize, usize, usize, usize)> = rooms.iter().map(|r| (r.1, r.2, r.3, r.4)).collect();
    find_vaults(level, width, room_borders)
}

// I originally had a floodfill check to make sure the level was fully connected 
// but after generating 100,000 levels and not hitting a single disjoint map, I 
//dropped the check.
pub fn draw_level(width: usize, height: usize) -> (Vec<Tile>, Vec<Vault>) {
    let mut level;
    
    // Loop unitl we generate a level with sufficient open space. 35% seems
    // to be decently full maps. Alternatively, I could have just kept trying
    // to add rooms until the level was sufficiently full, but this is simpler
    // and 80% of the time a generated map is more thna 35% open squares.
    let mut vaults;
    loop {
        level = Vec::new();
        for _ in 0..width*height {
            level.push(Tile::Wall);
        }

        vaults = carve(&mut level, width, height);

        let mut non_walls = 0;
        for &sq in &level {
            if sq != Tile::Wall {
                non_walls += 1;
            }
        }
        let ratio = non_walls as f32 / level.len() as f32;
        if ratio > 0.35 {
            break;
        }
    }

    (level, vaults)
}