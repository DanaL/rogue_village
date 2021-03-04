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
use std::collections::HashSet;

use crate::map;

// Kind of ugly by why recalculate these everytime?
#[inline]
fn radius_3() -> Vec<(i32, i32)> {
	let c = vec![(3, 0), (3, 0), (-3, 0), (-3, 0), (0, 3), (0, -3), (0, 3), (0, -3), (3, 1), (3, -1), 
		(-3, 1), (-3, -1), (1, 3), (1, -3), (-1, 3), (-1, -3), (2, 2), (2, -2), (-2, 2), (-2, -2), 
		(2, 2), (2, -2), (-2, 2), (-2, -2)];
	c	
}

#[inline]
fn radius_5() -> Vec<(i32, i32)> {
	let c = vec![(5, 0), (5, 0), (-5, 0), (-5, 0), (0, 5), (0, -5), (0, 5), (0, -5), (5, 1), (5, -1), 
		(-5, 1), (-5, -1), (1, 5), (1, -5), (-1, 5), (-1, -5), (5, 2), (5, -2), (-5, 2), (-5, -2), (2, 5), 
		(2, -5), (-2, 5), (-2, -5), (4, 3), (4, -3), (-4, 3), (-4, -3), (3, 4), (3, -4), (-3, 4), (-3, -4),
		(-3, -3), (3, 3), (-3, 3), (3, -3)];

	c	
}

#[inline]
fn radius_7() -> Vec<(i32, i32)> {
	let c = vec![(7, 0), (7, 0), (-7, 0), (-7, 0), (0, 7), (0, -7), (0, 7), (0, -7), (7, 1), (7, -1), (-7, 1), 
		(-7, -1), (1, 7), (1, -7), (-1, 7), (-1, -7), (7, 2), (7, -2), (-7, 2), (-7, -2), (2, 7), (2, -7), 
		(-2, 7), (-2, -7), (6, 3), (6, -3), (-6, 3), (-6, -3), (3, 6), (3, -6), (-3, 6), (-3, -6), (6, 4), 
		(6, -4), (-6, 4), (-6, -4), (4, 6), (4, -6), (-4, 6), (-4, -6), (5, 5), (5, -5), (-5, 5), (-5, -5), 
		(5, 5), (5, -5), (-5, 5), (-5, -5), (-4, -5), (4, 5), (-4, 5), (4, -5), (-5, -4), (5, 4), (-5, 4),
		(5, -4)];

	c
}

fn radius_9() -> Vec<(i32, i32)> {
	let c = vec![(9, 0), (9, 0), (-9, 0), (-9, 0), (0, 9), (0, -9), (0, 9), (0, -9), (9, 1), (9, -1), (-9, 1), 
		(-9, -1), (1, 9), (1, -9), (-1, 9), (-1, -9), (9, 2), (9, -2), (-9, 2), (-9, -2), (2, 9), (2, -9), 
		(-2, 9), (-2, -9), (9, 3), (9, -3), (-9, 3), (-9, -3), (3, 9), (3, -9), (-3, 9), (-3, -9), (8, 4), 
		(8, -4), (-8, 4), (-8, -4), (4, 8), (4, -8), (-4, 8), (-4, -8), (8, 5), (8, -5), (-8, 5), (-8, -5), 
		(5, 8), (5, -8), (-5, 8), (-5, -8), (7, 6), (7, -6), (-7, 6), (-7, -6), (6, 7), (6, -7), (-6, 7), 
		(-6, -7), (-6, -6), (6, 6), (6, -6), (-6, 6), (-7, -5), (7, 5), (-7, 5), (7, -5), (-5, -7), (5, 7),
		(-5, 7), (5, -7)];

	c
}

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
#[inline]
fn mark_visible(r1: i32, c1: i32, r2: i32, c2: i32, sq_radius: i32,
		depth: i8, visible: &mut HashSet<(i32, i32, i8)>, state: &GameState) {
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

			let dr = r - r1;
			let dc = c - c1;
			if dr * dr + dc * dc <= sq_radius || state.lit_sqs.contains(&(r, c, depth)) {
				visible.insert((r, c, depth));
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

			let dr = r - r1;
			let dc = c - c1;
			if dr * dr + dc * dc <= sq_radius || state.lit_sqs.contains(&(r, c, depth)) {
				visible.insert((r, c, depth));
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

// fov_only option is for when I want to check only what squares are visible from centre inside vision radius. When it's false, I also look
// for squares that are lit according to the list of lit sqs in GameState. (Ie., so the player may have no torch burning underground but can
// see the light from an independent light source)
pub fn calc_fov(state: &GameState, centre: (i32, i32, i8), radius: u8, fov_only: bool) -> HashSet<(i32, i32, i8)> {    
	// Even if the player's vision radius is only, say, 3 we still need to scan to the 
	// perimiter of the FOV area in case there are independently lit squares for which
	// the player has line-of-sight
	// To revisit: I bet this is quite likely a case of premature optimization
	let perimeter = if fov_only && radius <= 9 {
		if radius == 3 {
			radius_3()
		} else if radius == 5 {
			radius_5()
		} else if radius == 7 {
			radius_7()
		} else {
			radius_9()
		}
	} else {
		radius_full()
	};
	
    // Beamcast to all the points around the perimiter of the viewing
	// area. For RogueVillage's fixed size FOV this seems to work just fine in
	// terms of performance.
	let mut visible = HashSet::new();
	let sq_radius = radius as i32 * radius as i32 + 1;
	for loc in perimeter {
		let outer_r = centre.0 + loc.0;
		let outer_c = centre.1 + loc.1;

		mark_visible(centre.0, centre.1, outer_r as i32, outer_c as i32, sq_radius, centre.2, &mut visible, state);
	}

	visible
}

// Translates the set of visible squares into the grid used to select which tiles to show to the player
pub fn visible_sqs(state: &GameState, centre: (i32, i32, i8), radius: u8, fov_only: bool) -> Vec<((i32, i32, i8), bool)> {
	let visible = calc_fov(state, centre, radius, fov_only);

    // Now we know which locations are actually visible from the player's loc, 
    // copy the tiles into the v_matrix
    let mut v_matrix = Vec::with_capacity(FOV_HEIGHT * FOV_WIDTH);
	for r in centre.0 - 10..centre.0 + 11 {
		for c in centre.1 - 20..centre.1 + 21 {
			let loc = (r, c, centre.2);
			let visible = visible.contains(&loc);
			v_matrix.push((loc, visible));
		}
	}

	v_matrix
}
