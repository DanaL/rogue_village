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

use core::num;
use std::u16;

use rand::thread_rng;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::map::Tile;

fn pick_room() -> (Vec<Vec<Tile>>, u16, u16) {
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
        for j in 0..height {
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
                let d = (i32::abs(i32::pow(r as i32 - rc as i32, 2)) + i32::abs(i32::pow(c as i32 - cc as i32, 2))) as f64;
                if d.sqrt() <= radius as f64 {
                    room[r][c] = Tile::StoneFloor;
                }
            }
        }
    }

    (room, height as u16, width as u16)
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

fn room_fits(level: &mut Vec<Tile>, bounds: (i32, i32, i32, i32), width: usize) -> bool {    
    if bounds.0 <= 0 {
        return false;
    } else if bounds.1 as usize >= level.len() / width {
        return false;
    } else if bounds.2 <= 0 {
        return false;
    } else if bounds.3 as usize >= width {
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
            level[row * width + col] = Tile::Door(false);
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
        println!("fuckcunt {:?}", options);
        println!("cunt {} {} {} {}", lo, hi, row, col);
        if rng.gen_range(0.0, 1.0) < 0.8 {
            level[row * width + col] = Tile::Door(false);
        } else {
            level[row * width + col] = Tile::StoneFloor;
        }
    } else {
        // if there are no options for a 1-thickness wall to place an entranceway, make a short
        // hallway between the two rooms.        
        let row = (lo + hi) / 2;
        let mut col2 = col;
        println!("fuck {} {} {} {}", lo, hi, row, col);
        
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

fn place_room(level: &mut Vec<Tile>, rooms: &mut Vec<(Vec<Vec<Tile>>, u16, u16, u16, u16, &str)>,
    parent_index: usize, room: &(Vec<Vec<Tile>>, u16, u16), width: usize) -> bool {

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
                    rooms.push((new_room, start_row as u16, start_col as u16, end_row as u16, end_col as u16, "N"));
                    
                    let lo = if rooms[parent_index].2 + 1 > start_col as u16 {
                        rooms[parent_index].2 + 1
                    } else {
                        start_col as u16
                    };
                    let hi = if rooms[parent_index].4 - 1 < end_col as u16 {
                        rooms[parent_index].4 - 1
                    } else {
                        end_col as u16
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
                    rooms.push((new_room, start_row as u16, start_col as u16, end_row as u16, end_col as u16, "S"));
                    
                    let lo = if rooms[parent_index].2 + 1 > start_col as u16 {
                        rooms[parent_index].2 + 1
                    } else {
                        start_col as u16
                    };
                    let hi = if rooms[parent_index].4 - 1 < end_col as u16 {
                        rooms[parent_index].4 - 1
                    } else {
                        end_col as u16
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
                    rooms.push((new_room, start_row as u16, start_col as u16, end_row as u16, end_col as u16, "W"));
                    let lo = if rooms[parent_index].1 + 1 > start_row as u16 {
                        rooms[parent_index].1 + 1
                    } else {
                        start_row as u16
                    };
                    let hi = if rooms[parent_index].3 - 1 < end_row as u16 {
                        rooms[parent_index].3 - 1
                    } else {
                        end_row as u16
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
                    rooms.push((new_room, start_row as u16, start_col as u16, end_row as u16, end_col as u16, "E"));
                    let lo = if rooms[parent_index].1 + 1 > start_row as u16 {
                        rooms[parent_index].1 + 1
                    } else {
                        start_row as u16
                    };
                    let hi = if rooms[parent_index].3 - 1 < end_row as u16 {
                        rooms[parent_index].3 - 1
                    } else {
                        end_row as u16
                    };
                    add_doorway_vertical(level, start_col as usize, lo as usize, hi as usize, width);
                    return true;
                }
            }
        }

        //break;
    }
    
    false
}

fn find_spot_for_room(level: &mut Vec<Tile>, rooms: &mut Vec<(Vec<Vec<Tile>>, u16, u16, u16, u16, &str)>,
                            room: &(Vec<Vec<Tile>>, u16, u16), width: usize) -> bool {
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

fn carve(level: &mut Vec<Tile>, width: u16, height: u16) {
    let mut rooms = Vec::new();
    let mut rng = rand::thread_rng();    
    let center_row = (height / 2) as i16;
    let center_col = (width / 2) as i16;
    let row = (center_row + rng.gen_range(-10, 10)) as u16;
    let col = (center_col + rng.gen_range(-10, 10)) as u16;

    // Draw the starting room to the dungeon map. (This is just the first room we make on the
    // level, not necessaily the entrance room)
    let room = pick_room();
    draw_room(level, row as usize, col as usize, &room.0, width as usize);
    rooms.push((room.0, row, col, row + room.1, col + room.2, "Start"));

    loop {
        let room = pick_room();
        find_spot_for_room(level, &mut rooms, &room, width as usize);
        break;
    }
}

fn dump_level(level: &Vec<Tile>, width: usize, height: usize) {
    for r in 0..height {
        let mut s = String::from("");
        for c in 0..width {
            match level[width * r + c] {
                Tile::StoneFloor => s.push_str("."),
                Tile::Wall => s.push_str("#"),
                Tile::Door(true) => s.push_str("/"),
                Tile::Door(false) => s.push_str("+"),
                _ => s.push_str("?"),
            }            
        }
        println!("{}", s);
    }
}

pub fn make_level(width: u16, height: u16) -> Vec<Tile> {
    let mut level = Vec::new();
    
    for _ in 0..width*height {
        level.push(Tile::Wall);
    }

    carve(&mut level, width, height);

    dump_level(&level, width as usize, height as usize);
    level
}