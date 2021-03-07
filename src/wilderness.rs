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

use std::collections::HashMap;

use rand::{Rng, thread_rng};
use rand::seq::SliceRandom;

use super::Map;

use crate::map::Tile;
use crate::util;
use crate::world::WILDERNESS_SIZE;

fn fuzz() -> f64 {
	thread_rng().gen_range(-0.5, 0.5)
}

fn diamond_step(grid: &mut [f64], r: usize, c: usize, width: usize) {
	let avg = (grid[WILDERNESS_SIZE * r + c] + grid[WILDERNESS_SIZE * r + c + width - 1] +
					grid[(r + width - 1) * WILDERNESS_SIZE + c] + grid[(r + width - 1) * WILDERNESS_SIZE + c + width - 1]) / 4.0;
	
	grid[(r + width / 2) * WILDERNESS_SIZE + c + width / 2] = avg + fuzz();
}

fn calc_diamond_avg(grid: &mut [f64], r: usize, c: usize, width: usize) {
	let mut count = 0;
	let mut avg = 0.0;
	if width <= c {
		avg += grid[r * WILDERNESS_SIZE + c - width];
		count += 1;
	}
	if c + width < WILDERNESS_SIZE {
		avg += grid[r * WILDERNESS_SIZE + c + width];
		count += 1;
	}
	if width <= r {
		avg += grid[(r - width) * WILDERNESS_SIZE + c];
		count += 1;
	}
	if r + width < WILDERNESS_SIZE {
		avg += grid[(r + width) * WILDERNESS_SIZE + c];
		count += 1;
	}
	
	grid[r * WILDERNESS_SIZE + c] = avg / count as f64 + fuzz();
}

fn square_step(grid: &mut [f64], r: usize, c: usize, width: usize) {
	let half_width = width / 2;

	calc_diamond_avg(grid, r - half_width, c, half_width);
	calc_diamond_avg(grid, r + half_width, c, half_width);
	calc_diamond_avg(grid, r, c - half_width, half_width);
	calc_diamond_avg(grid, r, c + half_width, half_width);
}

fn midpoint_displacement(grid: &mut [f64], r: usize, c: usize, width: usize) {
	diamond_step(grid, r, c, width);
	let half_width = width / 2;
	square_step(grid, r + half_width, c + half_width, width);

	if half_width == 1 {
		return;
	}

	midpoint_displacement(grid, r, c, half_width + 1);
	midpoint_displacement(grid, r, c + half_width, half_width + 1);
	midpoint_displacement(grid, r + half_width, c, half_width + 1);
	midpoint_displacement(grid, r + half_width, c + half_width, half_width + 1);
}

// Average each point with its neighbours to smooth things out
fn smooth_map(grid: &mut [f64]) {
	for r in 0..WILDERNESS_SIZE {
		for c in 0..WILDERNESS_SIZE {
			let mut avg = grid[r * WILDERNESS_SIZE + c];
			let mut count = 1;

			if r >= 1 {
				if c >= 1 {
					avg += grid[(r - 1) * WILDERNESS_SIZE + c - 1];
					count += 1;
				}
				avg += grid[(r - 1) * WILDERNESS_SIZE + c];
				count += 1;
				if c + 1 < WILDERNESS_SIZE {
					avg += grid[(r - 1) * WILDERNESS_SIZE + c + 1];
					count += 1;
				}
			}

			if r > 1 && c >= 1 {
				avg += grid[(r - 1) * WILDERNESS_SIZE + c - 1];
				count += 1;
			}

			if r > 1 && c + 1 < WILDERNESS_SIZE {
				avg += grid[(r - 1) * WILDERNESS_SIZE + c + 1];
				count += 1;
			}

			if r > 1 && r + 1 < WILDERNESS_SIZE {
				if c >= 1 {
					avg += grid[(r - 1) * WILDERNESS_SIZE + c - 1];
					count += 1;
				}
				avg += grid[(r - 1) * WILDERNESS_SIZE + c];
				count += 1;
				if c + 1 < WILDERNESS_SIZE {
					avg += grid[(r - 1) * WILDERNESS_SIZE + c + 1];
					count += 1;
				}
			}

			grid[r * WILDERNESS_SIZE + c] = avg / count as f64;
		}
	}
}

fn translate_to_tile(grid: &[f64]) -> Map {
	let mut map = HashMap::new();

	for r in 0..WILDERNESS_SIZE {
		for c in 0..WILDERNESS_SIZE {
			if grid[r * WILDERNESS_SIZE + c] < 1.5 {
				map.insert((r as i32, c as i32, 0), Tile::DeepWater);
			} else if grid[r * WILDERNESS_SIZE + c] < 6.0 {
				map.insert((r as i32, c as i32, 0), Tile::Grass);
			} else {
				if thread_rng().gen_range(0.0, 1.0) < 0.9 {
					map.insert((r as i32, c as i32, 0), Tile::Mountain);
				} else {
					map.insert((r as i32, c as i32, 0), Tile::SnowPeak);
				}
			}
		}
	}

	map
}

fn bresenham(r0: i32, c0: i32, r1: i32, c1: i32) -> Vec<(i32, i32, i8)> {
	let mut pts = Vec::new();
	let mut error = 0;
	let mut r = r0;
	let mut c = c0;
	let mut delta_c = c1 - c0;
	
	let step_c = if delta_c < 0 {
		delta_c = -delta_c;
		-1
	} else {
		1
	};

	let mut delta_r = r1 - r0;
	let step_r = if delta_r < 0 {
		delta_r = -delta_r;
		-1
	} else {
		1
	};		

	if delta_r <= delta_c {
		let criterion = delta_c / 2;
		while c != c1 + step_c {
			pts.push((r, c, 0));
			c += step_c;
			error += delta_r;
			if error > criterion {
				error -= delta_c;
				r += step_r;
			}
		}
	} else {
		let criterion = delta_r / 2;
		while r != r1 + step_r {
			pts.push((r, c, 0));
			r += step_r;
			error += delta_c;
			if error > criterion {
				error -= delta_r;
				c += step_c;
			}
		}
	}

	pts
}

fn next_point(r: i32, c: i32, d: i32, angle: f64) -> (i32, i32, i8) {
	let next_r = r + (d as f64 * f64::sin(angle)) as i32;
	let next_c = c + (d as f64 * f64::cos(angle)) as i32;

	(next_r, next_c, 0)
}

fn draw_river(map: &mut Map, start: (i32, i32, i8), angle: f64) {
	let mut rng = rand::thread_rng();
	let mut row = start.0;
	let mut col = start.1;
	let mut pts = Vec::new();
	let mut curr_angle = angle;

	loop {
		let d = rng.gen_range(2, 5);
		let n = next_point(row, col, d, curr_angle);
		if !map.contains_key(&n) {
			break;
		}

		let next_segment = bresenham(row, col, n.0, n.1);
		let mut river_crossing = false;
		for pt in next_segment.iter() {
			pts.push((pt.0, pt.1, 0));
			if map[&(pt.0, pt.1, 0)] == Tile::DeepWater {
				river_crossing = true;
				break;
			} 
		}

		if map[&n] == Tile::DeepWater || river_crossing {
			break;
		}

		row = n.0;
		col = n.1;
		curr_angle += rng.gen_range(-0.2, 0.2);

		// keep the river from turning back and looking like it's flowing uphill in the mountains
		if curr_angle > -0.1 {
			curr_angle = -0.28;
		} else if angle < -0.3 {
			curr_angle = -2.6;
		}
	}

	// smooth river
	// bresenham draws lines that can look like:
	//     ~
	//   ~~
	//  ~@
	// I don't want those points where the player could walk
	// say NW and avoid stepping on the river
	let mut extra_pts = Vec::new();
	for x in 0..pts.len() - 1 {
		let a = pts[x];
		let b = pts[x + 1];
		if a.0 != b.0 && a.1 != b.1 {
			extra_pts.push((a.0 - 1, a.1, 0));
		}
	}

	for pt in pts.iter() {
		map.insert(*pt, Tile::DeepWater);
	}
	map.insert(start, Tile::DeepWater);

	for pt in extra_pts.iter() {
		map.insert(*pt, Tile::DeepWater);
	}
}

fn river_start(map: &Map, col_lo: usize, col_hi: usize) -> Option<(i32, i32, i8)> {
	let mut rng = rand::thread_rng();
	let x = WILDERNESS_SIZE / 3;

	loop {
		let r = rng.gen_range(WILDERNESS_SIZE - x, WILDERNESS_SIZE - 2);
		let c = rng.gen_range(col_lo, col_hi);

		let mountains = count_neighbouring_terrain(map, (r as i32, c as i32, 0), Tile::Mountain);
		if mountains > 3 {
			return Some((r as i32, c as i32, 0));
		}
	}
}

fn draw_rivers(map: &mut Map) {
	// Try to draw up to three rivers on the map
	let mut rng = rand::thread_rng();
	let mut opts = [0, 1, 2];
	opts.shuffle(&mut rng);

	let mut passes = 0;
	for opt in opts.iter() {
		if passes == 0 || rand::thread_rng().gen_range(0.0, 1.0) < 0.5 {
			if *opt == 0 {
				if let Some(loc) = river_start(map, 2, WILDERNESS_SIZE / 3) {
					let angle = -0.28;
					draw_river(map, loc, angle);
				}
			} else if *opt == 1 {
				if let Some(loc) = river_start(map, WILDERNESS_SIZE / 3, (WILDERNESS_SIZE / 3) * 2) {
					let angle = -1.5;
					draw_river(map, loc, angle);
				}
			} else {
				if let Some(loc) = river_start(map, WILDERNESS_SIZE - WILDERNESS_SIZE / 3 - 2, WILDERNESS_SIZE - 2) {
					let angle = -2.5;
					draw_river(map, loc, angle);
				}
			}
		}
		passes += 1;
	}
}

fn count_neighbouring_terrain(map: &Map, loc: (i32, i32, i8), tile: Tile) -> u32 {
	let mut count = 0;

	for a in util::ADJ.iter() {
		let nl = (loc.0 + a.0, loc.1 + a.1, loc.2);
		if map.contains_key(&nl) && map[&nl] == tile {
			count += 1;
		}
	}

	count
}

// Lay down trees using a cellular automata rule starting with a
// 50/50 mix of trees and grass
fn lay_down_trees(map: &mut Map) -> Map {
	let keys = map.keys()
				  .map(|k| k.clone())
				  .collect::<Vec<(i32, i32, i8)>>();
	
	for k in &keys {
		if map[&k] == Tile::Grass && thread_rng().gen_range(0.0, 1.0) < 0.5 {
			map.insert(*k, Tile::Tree);
		}
	}
	
	// Two generations seems to make a nice mix of trees and grass
	for _ in 0..2 {
		let mut next_gen = map.clone();

		for k in &keys {
			if map[k] == Tile::Grass {
				let trees = count_neighbouring_terrain(&map, *k, Tile::Tree);
				if trees >= 6 && trees <= 8 {
					next_gen.insert(*k, Tile::Tree);
				}
			} else if map[k] == Tile::Tree {
				let trees = count_neighbouring_terrain(&map, *k, Tile::Tree);
				if trees < 4  {
					next_gen.insert(*k, Tile::Grass);
				}
			}
		}
		
		for k in &keys {
			map.insert(*k, next_gen[&k]);
		}
	}

	let mut result = HashMap::new();
	for k in &keys {
		result.insert(*k, map[&k]);
	}

	result
}

fn draw_borders(map: &mut Map) {
	let mut rng = rand::thread_rng();
	for col in 0..WILDERNESS_SIZE {
		for row in 0..rng.gen_range(5, 11) {
			map.insert((row as i32, col as i32, 0), Tile::DeepWater);
		}
		map.insert((WILDERNESS_SIZE as i32 - 1, col as i32, 0), Tile::Mountain);
	}

	let x = rng.gen_range(WILDERNESS_SIZE / 3, WILDERNESS_SIZE / 3 * 2);
	for r in 0..x {
		map.insert((r as i32, 0, 0), Tile::WorldEdge);
	}
	for r in x..WILDERNESS_SIZE {
		map.insert((r as i32, 0, 0), Tile::Mountain);
	}
	let x = rng.gen_range(WILDERNESS_SIZE / 3, WILDERNESS_SIZE / 3 * 2);
	for r in 0..x {
		map.insert((r as i32, WILDERNESS_SIZE as i32 - 1, 0), Tile::WorldEdge);
	}
	for r in x..WILDERNESS_SIZE {
		map.insert((r as i32, WILDERNESS_SIZE as i32 - 1, 0), Tile::Mountain);
	}
}

pub fn gen_wilderness_map() -> Map {
	let mut grid: [f64; WILDERNESS_SIZE * WILDERNESS_SIZE] = [0.0; WILDERNESS_SIZE * WILDERNESS_SIZE];
	grid[0] = thread_rng().gen_range(-1.0, 1.0);
	grid[WILDERNESS_SIZE - 1] = thread_rng().gen_range(1.0, 2.5);
	grid[(WILDERNESS_SIZE - 1) * WILDERNESS_SIZE] = thread_rng().gen_range(10.0, 12.0);
	grid[ WILDERNESS_SIZE * WILDERNESS_SIZE - 1] = thread_rng().gen_range(9.0, 11.0);

	midpoint_displacement(&mut grid, 0, 0, WILDERNESS_SIZE);
	smooth_map(&mut grid);

	let mut map = translate_to_tile(&grid);
	lay_down_trees(&mut map);
	draw_rivers(&mut map);
	draw_borders(&mut map);

	map
}
