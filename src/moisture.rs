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
    pub rain_rate: Vec<f32>,
    pub rainfall: Vec<f32>,
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
            rain_rate: vec![0.; WIDTH * HEIGHT],
            rainfall: vec![0.; WIDTH * HEIGHT],
            running: false,
            visible: true,
            elapsed: 0.,
        }
    }
}

impl Moisture {
    pub fn farm_rainfall(&self) -> f32 {
        let mut sum = 0.;
        let mut count = 0;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let p = position(x, y);
                if (p.x - 340.).abs() <= 38. && p.y.abs() <= 30. {
                    sum += self.rainfall[y * WIDTH + x];
                    count += 1;
                }
            }
        }
        sum / count as f32
    }

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

    #[cfg(test)]
    pub fn advance(&mut self, velocity: impl Fn(Vec2) -> Vec2) {
        self.advance_with_rain(velocity, |_| 0., |_| 0.);
    }

    pub fn advance_with_rain(
        &mut self,
        velocity: impl Fn(Vec2) -> Vec2,
        height: impl Fn(Vec2) -> f32,
        low_lift: impl Fn(Vec2) -> f32,
    ) {
        if !self.running {
            return;
        }
        let mut next = vec![0.; WIDTH * HEIGHT];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let p = position(x, y);
                let upstream = p - velocity(p) * DT;
                let transported = self.sample(upstream);
                // Sea replenishes humidity towards saturation at 0.8 per second.
                next[y * WIDTH + x] = if p.x < -225. {
                    transported + (1. - transported) * (1. - (-0.8 * DT).exp())
                } else {
                    transported
                }
                .clamp(0., 1.);
                // Positive altitude change is uplift; descent does not produce rain.
                let uplift = ((height(p) - height(upstream)) / DT).max(0.) + low_lift(p).max(0.);
                let available = (next[y * WIDTH + x] - 0.25).max(0.);
                let condensed = available * (1. - (-uplift * 3. * DT).exp());
                next[y * WIDTH + x] -= condensed;
                self.rain_rate[y * WIDTH + x] = condensed / DT;
                self.rainfall[y * WIDTH + x] += condensed;
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
    fn uphill_rain_consumes_moisture_and_downhill_stays_dry() {
        let mut up = Moisture::default();
        up.values.fill(0.9);
        up.running = true;
        let mut down = Moisture::default();
        down.values.fill(0.9);
        down.running = true;
        let i = 9 * WIDTH + 15;
        up.advance_with_rain(|_| Vec2::new(30., 0.), |p| p.x / 100., |_| 0.);
        down.advance_with_rain(|_| Vec2::new(-30., 0.), |p| p.x / 100., |_| 0.);
        assert!(up.rain_rate[i] > 0.);
        assert!((up.values[i] + up.rainfall[i] - 0.9).abs() < 1e-5);
        assert_eq!(down.rain_rate[i], 0.);
        assert_eq!(down.rainfall[i], 0.);
    }

    #[test]
    fn dry_air_cannot_rain_and_pause_freezes_accumulation() {
        let mut m = Moisture::default();
        m.values.fill(0.);
        m.running = true;
        m.advance_with_rain(|_| Vec2::ZERO, |_| 0., |_| 1.);
        assert_eq!(m.rain_rate[15], 0.);
        m.values.fill(0.9);
        m.advance_with_rain(|_| Vec2::ZERO, |_| 0., |_| 1.);
        assert!(m.farm_rainfall() > 0.);
        let before = m.rainfall.clone();
        m.running = false;
        m.advance_with_rain(|_| Vec2::ZERO, |_| 0., |_| 1.);
        assert_eq!(before, m.rainfall);
        assert_eq!(Moisture::default().farm_rainfall(), 0.);
    }

    #[test]
    fn pressure_placement_changes_rainfall() {
        let mut a = Moisture::default();
        let mut b = Moisture::default();
        a.running = true;
        b.running = true;
        for _ in 0..1800 {
            a.advance_with_rain(
                |p| crate::wind_at(p, crate::INITIAL_LOW) * 75.,
                crate::terrain_height,
                |p| crate::low_uplift(p, crate::INITIAL_LOW),
            );
            b.advance_with_rain(
                |p| crate::wind_at(p, Vec2::new(350., -100.)) * 75.,
                crate::terrain_height,
                |p| crate::low_uplift(p, Vec2::new(350., -100.)),
            );
        }
        println!(
            "60s farm rain: initial={:.4}, southeast={:.4} game units",
            a.farm_rainfall(),
            b.farm_rainfall()
        );
        assert!(a.rainfall.iter().any(|v| *v > 0.1));
        assert!((a.farm_rainfall() - b.farm_rainfall()).abs() > 0.001);
        assert!(
            a.values
                .iter()
                .chain(&b.values)
                .all(|v| v.is_finite() && (0. ..=1.).contains(v))
        );
    }

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
