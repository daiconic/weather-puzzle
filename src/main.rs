use bevy::prelude::*;
mod moisture;
use moisture::Moisture;

const HIGH: Vec2 = Vec2::new(-350., 110.);
const INITIAL_LOW: Vec2 = Vec2::new(130., 150.);
const MAP_HALF: Vec2 = Vec2::new(480., 270.);
const LOW_COLOR: Color = Color::srgb(1., 0.52, 0.36);
const HIGH_COLOR: Color = Color::srgb(0.48, 0.78, 1.);

#[derive(Resource)]
struct Weather {
    low: Vec2,
    drag_offset: Option<Vec2>,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            low: INITIAL_LOW,
            drag_offset: None,
        }
    }
}

#[derive(Component)]
struct LowLabel;

#[derive(Component)]
struct HumidityCell(usize);

#[derive(Component)]
struct Status;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.035, 0.06, 0.09)))
        .init_resource::<Weather>()
        .init_resource::<Moisture>()
        .insert_resource(Time::<Fixed>::from_hz(30.))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Weather Lab | 02 - Moisture".into(),
                resolution: (1100, 760).into(),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(PreUpdate, controls.after(bevy::input::InputSystems))
        .add_systems(FixedUpdate, transport)
        .add_systems(Update, (drag_low, draw_weather, show_moisture).chain())
        .run();
}

fn label(commands: &mut Commands, text: &str, position: Vec2, size: f32, color: Color) -> Entity {
    commands
        .spawn((
            Text2d::new(text),
            TextFont {
                font_size: bevy::text::FontSize::Px(size),
                ..default()
            },
            TextColor(color),
            Transform::from_translation(position.extend(5.)),
        ))
        .id()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    let white = Color::srgb(0.88, 0.94, 0.95);
    label(
        &mut commands,
        "WEATHER LAB",
        Vec2::new(-330., 330.),
        30.,
        white,
    );
    label(
        &mut commands,
        "02 / CARRY THE MOISTURE",
        Vec2::new(310., 330.),
        19.,
        HIGH_COLOR,
    );
    label(
        &mut commands,
        "Drag L  /  Space: play-pause  /  R: reset  /  M: moisture",
        Vec2::new(0., -315.),
        20.,
        white,
    );
    label(
        &mut commands,
        "Cyan = moisture: dark 0% -> bright 100%. Terrain does not affect flow yet.",
        Vec2::new(0., -345.),
        16.,
        Color::srgb(0.53, 0.65, 0.70),
    );

    let status = label(
        &mut commands,
        "PAUSED | 0.0s | Farm moisture: 0%",
        Vec2::new(0., 290.),
        17.,
        white,
    );
    commands.entity(status).insert(Status);
    // An independent transparent layer keeps the underlying terrain inspectable.
    for y in 0..moisture::HEIGHT {
        for x in 0..moisture::WIDTH {
            let p = moisture::position(x, y);
            let ridge = terrain_height(p);
            let color = if p.x < -225. {
                Color::srgb(0.07, 0.23 + 0.025 * (y % 3) as f32, 0.34)
            } else {
                Color::srgb(
                    0.15 + ridge * 0.30,
                    0.28 + ridge * 0.20,
                    0.24 + ridge * 0.24,
                )
            };
            commands.spawn((
                Sprite::from_color(color, Vec2::splat(29.)),
                Transform::from_translation(p.extend(0.)),
            ));
            commands.spawn((
                Sprite::from_color(Color::NONE, Vec2::splat(29.)),
                Transform::from_translation(p.extend(1.)),
                HumidityCell(y * moisture::WIDTH + x),
            ));
        }
    }
    label(
        &mut commands,
        "SEA",
        Vec2::new(-355., -220.),
        18.,
        HIGH_COLOR,
    );
    label(&mut commands, "RIDGE", Vec2::new(-20., 230.), 16., white);
    label(
        &mut commands,
        "SOUTH PASS",
        Vec2::new(-15., -220.),
        15.,
        white,
    );
    label(
        &mut commands,
        "FARM",
        Vec2::new(340., -55.),
        18.,
        Color::srgb(1., 0.84, 0.43),
    );
    label(&mut commands, "H", HIGH, 30., HIGH_COLOR);
    let low = label(&mut commands, "L", INITIAL_LOW, 30., LOW_COLOR);
    commands.entity(low).insert(LowLabel);
}

fn terrain_height(p: Vec2) -> f32 {
    let ridge = (-((p.x + 20.) / 62.).powi(2)).exp();
    ridge * ((p.y + 170.) / 95.).clamp(0., 1.)
}

// A deliberately simple game field, not an atmospheric forecast:
// broad outward flow around H plus inward flow around L and a small westerly.
fn wind_at(p: Vec2, low: Vec2) -> Vec2 {
    let away = p - HIGH;
    let toward = low - p;
    Vec2::new(0.20, 0.)
        + away / (away.length_squared() + 18000.) * 230.
        + toward / (toward.length_squared() + 18000.) * 300.
}

fn drag_low(
    mut weather: ResMut<Weather>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut low_label: Single<&mut Transform, With<LowLabel>>,
) {
    if !mouse.pressed(MouseButton::Left) || !window.focused {
        weather.drag_offset = None;
    } else if let Some(cursor) = window.cursor_position()
        && let Ok(p) = camera.0.viewport_to_world_2d(camera.1, cursor)
    {
        if mouse.just_pressed(MouseButton::Left) && p.distance(weather.low) <= 34. {
            weather.drag_offset = Some(weather.low - p);
        }
        if let Some(offset) = weather.drag_offset {
            weather.low =
                (p + offset).clamp(-MAP_HALF + Vec2::splat(38.), MAP_HALF - Vec2::splat(38.));
        }
    }
    low_label.translation = weather.low.extend(5.);
}

fn draw_weather(mut gizmos: Gizmos, weather: Res<Weather>) {
    // Lift lines above the tile sprites with 3D line segments in the 2D camera.
    for y in 0..12 {
        for x in 0..21 {
            let p = Vec2::new(-450. + x as f32 * 45., -247.5 + y as f32 * 45.);
            if p.distance(weather.low) < 45. || p.distance(HIGH) < 45. {
                continue;
            }
            let wind = wind_at(p, weather.low);
            let length = (wind.length() * 19.).min(34.);
            let d = wind.normalize_or_zero();
            let end = p + d * length;
            let side = Vec2::new(-d.y, d.x);
            let color = Color::srgba(0.72, 0.89, 0.88, 0.72);
            for (a, b) in [
                (p, end),
                (end, end - d * 6. + side * 3.),
                (end, end - d * 6. - side * 3.),
            ] {
                gizmos.line(a.extend(2.), b.extend(2.), color);
            }
        }
    }
    for (center, color) in [(HIGH, HIGH_COLOR), (weather.low, LOW_COLOR)] {
        for radius in [34., 57., 80.] {
            for i in 0..64 {
                let a = i as f32 * std::f32::consts::TAU / 64.;
                let b = (i + 1) as f32 * std::f32::consts::TAU / 64.;
                gizmos.line(
                    (center + Vec2::from_angle(a) * radius).extend(3.),
                    (center + Vec2::from_angle(b) * radius).extend(3.),
                    color,
                );
            }
        }
    }
    let farm = Vec2::new(340., 0.);
    for y in [-30., -15., 0., 15., 30.] {
        gizmos.line(
            (farm + Vec2::new(-38., y)).extend(3.),
            (farm + Vec2::new(38., y)).extend(3.),
            Color::srgb(1., 0.84, 0.43),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_low_changes_farm_wind() {
        let farm = Vec2::new(340., 0.);
        let before = wind_at(farm, INITIAL_LOW);
        let after = wind_at(farm, Vec2::new(350., -200.));
        assert!(before.y > 0. && after.y < 0.);
    }

    #[test]
    fn wind_remains_finite_at_pressure_centers() {
        for p in [HIGH, INITIAL_LOW, Vec2::ZERO] {
            assert!(wind_at(p, INITIAL_LOW).is_finite());
        }
    }
}

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    mut weather: ResMut<Weather>,
    mut moisture: ResMut<Moisture>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        *weather = Weather::default();
        *moisture = Moisture::default();
        return;
    }
    if keys.just_pressed(KeyCode::Space) {
        moisture.running = !moisture.running;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        moisture.visible = !moisture.visible;
    }
}

fn transport(weather: Res<Weather>, mut moisture: ResMut<Moisture>) {
    moisture.advance(|p| wind_at(p, weather.low) * 75.);
}

fn show_moisture(
    moisture: Res<Moisture>,
    mut cells: Query<(&HumidityCell, &mut Sprite)>,
    mut status: Single<&mut Text2d, With<Status>>,
) {
    for (cell, mut sprite) in &mut cells {
        let alpha = if moisture.visible {
            moisture.values[cell.0] * 0.8
        } else {
            0.
        };
        sprite.color = Color::srgba(0.15, 0.85, 0.95, alpha);
    }
    status.0 = format!(
        "{} | {:.1}s | Farm moisture: {:.0}% | Overlay: {}",
        if moisture.running {
            "RUNNING"
        } else {
            "PAUSED"
        },
        moisture.elapsed,
        moisture.sample(Vec2::new(340., 0.)) * 100.,
        if moisture.visible { "ON" } else { "OFF" }
    );
}
