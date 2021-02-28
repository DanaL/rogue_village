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

use super::{FOV_WIDTH, FOV_HEIGHT, GameState};

use crate::map;
use crate::player::Player;
use crate::util;

#[inline]
fn radius_full() -> Vec<(i32, i32)> {
	let mut c = Vec::new();
	let width_radius = (FOV_WIDTH / 2) as i32;
	let height_radius = (FOV_HEIGHT / 2) as i32;

	for col in -width_radius..width_radius {
		c.push((-height_radius, col));
		c.push((height_radius, col));
	}

	for row in -height_radius..height_radius {
		c.push((row, -width_radius));
		c.push((row, width_radius));
	}

	c.push((height_radius, width_radius));

	c	
}

// Using bresenham line casting to detect blocked squares. If a ray hits
// a Wall before reaching target then we can't see it. Bresenham isn't 
// really a good way to do this because it leaves blindspots the further
// away you get and also is rather ineffecient (you visit the same squares 
// several times). My original plan, after making a prototype with beamcasting,
// was to switch to shadowcasting. But bresenham seemed sufficiently fast
// and I haven't seen any blindspots (perhaps because I'm keeping the FOV at
// 40x20).
//
// As well, I wanted to have the trees obscure/reduce the FOV instead of outright
// blocking vision and I couldn't think of a simple way to do that with 
// shadowcasting.
fn mark_visible(r1: i32, c1: i32, r2: i32, c2: i32, radius: f64,
		depth: i8, v_matrix: &mut Vec<bool>, 
        state: &GameState, width: usize) {
    let mut r = r1;
	let mut c = c1;
	let mut error = 0;

	let mut r_step = 1;
	let mut delta_r = r2 - r;
	if delta_r < 0 {
		delta_r = -delta_r;
		r_step = -1;
	} 

	let mut c_step = 1;
	let mut delta_c = c2 - c;
	if delta_c < 0 {
		delta_c = -delta_c;
		c_step = -1;
	} 

	let mut r_end = r2;
	let mut c_end = c2;
	if delta_c <= delta_r {
		let criterion = delta_r / 2;
		loop {
			if r_step > 0 && r >= r_end + r_step {
				break;
			} else if r_step < 0 && r <= r_end + r_step {
				break;
			}

			if !state.map.contains_key(&(r, c, depth)) {
				return;
			}

			if util::distance(r1, c1, r, c).round() <= radius || state.lit_sqs.contains(&(r, c, depth)) {
				let vm_r = r - r1 + 10;
				let vm_c = c - c1 + 20;
				let vmi = (vm_r * width as i32 + vm_c) as usize;
				v_matrix[vmi] = true;
			}

			if !state.map[&(r, c, depth)].clear() {
				return;
			}

			// I want trees to not totally block light, but instead reduce visibility, but fog 
            // completely blocks light.           
			if map::Tile::Tree == state.map[&(r, c, depth)] && !(r == r1 && c == c1) {
				if r_step > 0 {
					r_end -= 2;
				} else {
					r_end += 2;
				}
			}

			r += r_step;
			error += delta_c;
			if error > criterion {
				error -= delta_r;
				c += c_step;
			}
		} 	
	} else {
		let criterion = delta_c / 2;
		loop {
			if c_step > 0 && c >= c_end + c_step {
				break;
			} else if c_step < 0 && c <= c_end + c_step {
				break;
			}

			if !state.map.contains_key(&(r, c, depth)) {
				return;
			}

			if util::distance(r1, c1, r, c).round() <= radius || state.lit_sqs.contains(&(r, c, depth)) {
				let vm_r = r - r1 + 10;
				let vm_c = c - c1 + 20;
				let vmi = (vm_r * width as i32 + vm_c) as usize;
				v_matrix[vmi] = true;
			}

			if !state.map[&(r, c, depth)].clear() {
				return;
			}

			// Same as above, trees partially block vision instead of cutting it off
            //if curr_weather.clouds.contains(&(r as usize, c as usize)) && !no_fog.contains(&(r as usize, c as usize)) {
            if map::Tile::Tree == state.map[&(r, c, depth)] && !(r == r1 && c == c1) {
				if c_step > 0 {
					c_end -= 2;
				} else {
					c_end += 2;
				}
			}
			
			c += c_step;
			error += delta_r;
			if error > criterion {
				error -= delta_c;
				r += r_step;
			}
		}
	}
}

pub fn calc_fov(state: &GameState, centre: (i32, i32, i8), radius: u8, height: usize, width: usize) -> Vec<((i32, i32, i8), bool)> {
    let size = height * width;
    let mut visible = vec![false; size];
	let fov_center_r = height / 2;
	let fov_center_c = width / 2;
	visible[fov_center_r * width + fov_center_c] = true;

	let perimeter = radius_full();
	
    let pr = centre.0;
    let pc = centre.1;
	// Beamcast to all the points around the perimiter of the viewing
	// area. For RogueVillage's fixed size FOV this seems to work just fine in
	// terms of performance.
	for loc in perimeter {
		let actual_r = pr + loc.0;
		let actual_c = pc + loc.1;

		mark_visible(pr, pc, actual_r as i32, actual_c as i32, radius as f64, centre.2, &mut visible, state, width);
	}

    // Now we know which locations are actually visible from the player's loc, 
    // copy the tiles into the v_matrix
    let mut v_matrix = Vec::with_capacity(size);
    for r in 0..height {
        for c in 0..width {
            let j = r * width + c;
			let row = pr - fov_center_r as i32 + r as i32;
            let col = pc - fov_center_c as i32 + c as i32;
			let loc = (row, col, centre.2);
			v_matrix.push((loc, visible[j]));
        }
    }

	v_matrix
}
