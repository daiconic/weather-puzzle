//! Game-scale humidity tracer. Semi-Lagrangian transport is bounded but not mass-conserving.
use bevy::prelude::*;

pub const WIDTH: usize = 32;
pub const HEIGHT: usize = 18;
pub const CELL: f32 = 30.;
pub const DT: f32 = 1. / 30.;

pub fn position(x: usize, y: usize) -> Vec2 {
    Vec2::new(-465. + x as f32 * CELL, -255. + y as f32 * CELL)
}

#[derive(Resource)]
pub struct Moisture {
    pub values: Vec<f32>,
    pub running: bool,
    pub visible: bool,
    pub elapsed: f32,
}

impl Default for Moisture {
    fn default() -> Self {
        Self {
            values: (0..WIDTH * HEIGHT)
                .map(|i| {
                    if position(i % WIDTH, i / WIDTH).x < -225. {
                        1.
                    } else {
                        0.
                    }
                })
                .collect(),
            running: false,
            visible: true,
            elapsed: 0.,
        }
    }
}

impl Moisture {
    pub fn sample(&self, p: Vec2) -> f32 {
        let grid = (p - position(0, 0)) / CELL;
        let x = grid.x.floor() as i32;
        let y = grid.y.floor() as i32;
        // Air entering from outside the map is dry; outgoing air leaves freely.
        let at = |x: i32, y: i32| {
            if x < 0 || y < 0 || x >= WIDTH as i32 || y >= HEIGHT as i32 {
                0.
            } else {
                self.values[y as usize * WIDTH + x as usize]
            }
        };
        let fx = grid.x - x as f32;
        let fy = grid.y - y as f32;
        let a = at(x, y) * (1. - fx) + at(x + 1, y) * fx;
        let b = at(x, y + 1) * (1. - fx) + at(x + 1, y + 1) * fx;
        a * (1. - fy) + b * fy
    }

    pub fn advance(&mut self, velocity: impl Fn(Vec2) -> Vec2) {
        if !self.running {
            return;
        }
        let mut next = vec![0.; WIDTH * HEIGHT];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let p = position(x, y);
                let transported = self.sample(p - velocity(p) * DT);
                // Sea replenishes humidity towards saturation at 0.8 per second.
                next[y * WIDTH + x] = if p.x < -225. {
                    transported + (1. - transported) * (1. - (-0.8 * DT).exp())
                } else {
                    transported
                }
                .clamp(0., 1.);
            }
        }
        self.values = next;
        self.elapsed += DT;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_moves_one_cell_downwind_without_in_place_smearing() {
        let mut m = Moisture::default();
        m.values.fill(0.);
        m.values[9 * WIDTH + 15] = 1.;
        m.running = true;
        m.advance(|_| Vec2::new(CELL / DT, 0.));
        assert!((m.values[9 * WIDTH + 16] - 1.).abs() < 1e-5);
        assert_eq!(m.values[9 * WIDTH + 15], 0.);
        assert_eq!(m.values[9 * WIDTH + 17], 0.);
    }

    #[test]
    fn paused_state_is_unchanged_and_sea_is_the_only_source() {
        let mut m = Moisture::default();
        m.values.fill(0.);
        m.advance(|_| Vec2::ZERO);
        assert_eq!(m.elapsed, 0.);
        assert!(m.values.iter().all(|v| *v == 0.));
        m.running = true;
        m.advance(|_| Vec2::ZERO);
        assert!(m.values[0] > 0.);
        assert_eq!(m.values[15], 0.);
    }

    #[test]
    fn pressure_intervention_changes_transport_and_stays_bounded() {
        let mut a = Moisture::default();
        let mut b = Moisture::default();
        a.running = true;
        b.running = true;
        for _ in 0..900 {
            a.advance(|p| crate::wind_at(p, crate::INITIAL_LOW) * 75.);
            b.advance(|p| crate::wind_at(p, Vec2::new(350., -200.)) * 75.);
        }
        let difference: f32 = a
            .values
            .iter()
            .zip(&b.values)
            .map(|(a, b)| (a - b).abs())
            .sum();
        println!(
            "30s pressure intervention: mean humidity difference = {:.4}",
            difference / (WIDTH * HEIGHT) as f32
        );
        assert!(difference > 5.);
        assert!(
            a.values
                .iter()
                .chain(&b.values)
                .all(|v| v.is_finite() && (0. ..=1.).contains(v))
        );
        assert!(
            a.values
                .iter()
                .enumerate()
                .any(|(i, v)| i % WIDTH > 8 && *v > 0.1)
        );
    }
}
