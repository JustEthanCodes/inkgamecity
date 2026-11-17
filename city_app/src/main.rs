//! City Floorlines — game binary (composition root).
//!
//! Phase 1 vertical slice: procedural heightfield terrain, scattered building boxes,
//! sun + shadows, PanOrbit-style camera input (mouse-drag orbit, wheel zoom, WASD pan),
//! sim clock ticking in FixedUpdate. Set CFL_EXIT_FRAMES=<n> to auto-exit after n frames (CI smoke test).

use bevy::prelude::*;
use city_render::{CityRenderPlugin, RenderSettings};
use city_sim::{BuildStats, CitySimPlugin, MunicipalBudget, SimulationClock};
use city_terrain::Heightfield;
use city_ui::CityUiPlugin;

const TERRAIN_CELLS: u32 = 384; // 384 × 384 cells × 4 m ≈ 1.5 km view — cheap first run
const TERRAIN_CELL_M: f32 = 4.0;
const WORLD_SEED: u64 = 0xC1_11_5EED; // a tip of the cap to classic citybuilders

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        title: "City Floorlines".into(),
        ..default()
    }))
    .add_plugins((CitySimPlugin, CityRenderPlugin, CityUiPlugin))
    .add_systems(Startup, setup)
    .add_systems(Update, (camera_input, log_clock_progress, auto_exit))
    .run();
}

#[derive(Component)]
struct OrbitTarget(Vec3);

#[derive(Resource)]
struct FrameCounter(u32);

#[derive(Component)]
struct BuildingMarker;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let _ = asset_server; // Phase 2+: BSN/RON scenes and KTX2 atlases load here.
    commands.insert_resource(FrameCounter(0));
    commands.insert_resource(RenderSettings::default());

    let heightfield = Heightfield::procedural(TERRAIN_CELLS, TERRAIN_CELLS, TERRAIN_CELL_M, WORLD_SEED);

    // ---- Terrain mesh from the heightfield (single LOD for now; chunking is Phase 1-cont.)
    let terrain_mesh = build_terrain_mesh(&heightfield);
    let terrain_handle = meshes.add(terrain_mesh);
    commands.spawn((
        Mesh3d(terrain_handle),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.38, 0.18),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::IDENTITY,
    ));

    // ---- Procedural buildings (boxes) on a jittered grid, height from LCG, deterministic
    let concrete = materials.add(StandardMaterial { base_color: Color::srgb(0.62, 0.60, 0.58), perceptual_roughness: 0.8, ..default() });
    let brick = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.34, 0.28), perceptual_roughness: 0.85, ..default() });
    let glass = materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.65, 0.78), perceptual_roughness: 0.15, metallic: 0.4, ..default() });

    let mut rng_state: u64 = 0x853c_49e6_748f_ea9bu64;
    let mut lcg = move || {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (rng_state >> 11) as f32 / (1u64 << 53) as f32
    };

    for gx in -14i32..=14 {
        for gz in -14i32..=14 {
            let jitter_x = (lcg() - 0.5) * 14.0;
            let jitter_z = (lcg() - 0.5) * 14.0;
            let x = gx as f32 * 28.0 + jitter_x;
            let z = gz as f32 * 28.0 + jitter_z;
            if x.abs() > 360.0 || z.abs() > 360.0 {
                continue; // leave the river valley edges wilder
            }
            let roll = lcg();
            let height = if roll < 0.55 { 8.0 + lcg() * 18.0 } else if roll < 0.9 { 20.0 + lcg() * 45.0 } else { 55.0 + lcg() * 70.0 };
            let footprint = 9.0 + lcg() * 10.0;
            let ground = heightfield.height(glam::Vec2::new(x, z));
            let material = if roll < 0.35 { brick.clone() } else if roll < 0.85 { concrete.clone() } else { glass.clone() };
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(footprint, height, footprint))),
                MeshMaterial3d(material),
                Transform::from_xyz(x, ground + height * 0.5, z),
                BuildingMarker,
            ));
        }
    }

    // ---- Sun with shadows
    commands.spawn((
        DirectionalLight {
            illuminance: 90_000.0,
            shadows_enabled: true,
            shadow_depth_bias: 0.15,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.6, 0.0)),
    ));

    // ---- Camera: high orbit view (PanOrbitCamera extension lands in Phase 4)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(420.0, 330.0, 420.0).looking_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y),
        OrbitTarget(Vec3::new(0.0, 20.0, 0.0)),
    ));

    info!("City Floorlines — world seed {WORLD_SEED:#x}, terrain {TERRAIN_CELLS}² × {TERRAIN_CELL_M} m");
}

fn build_terrain_mesh(hf: &Heightfield) -> Mesh {
    let cells = hf.cells_x as usize;
    let cell = hf.cell_size;
    let half = (hf.cells_x as f32 * cell) * 0.5;
    let mut positions = Vec::with_capacity(cells * cells * 3);
    let mut normals = Vec::with_capacity(cells * cells * 3);
    for y in 0..hf.cells_y {
        for x in 0..hf.cells_x {
            let world = glam::Vec2::new(x as f32 * cell - half, y as f32 * cell - half);
            let h = hf.height(world);
            positions.push([world.x, h, world.y]);
            let n = hf.normal(world);
            normals.push([n.x, n.y, n.z]);
        }
    }
    let mut indices = Vec::with_capacity((cells - 1) * (cells - 1) * 6);
    for y in 0..(hf.cells_y as usize - 1) {
        for x in 0..(cells - 1) {
            let i00 = (y * cells + x) as u32;
            let i10 = i00 + 1;
            let i01 = i00 + cells as u32;
            let i11 = i01 + 1;
            indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
        }
    }
    let mut mesh = Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::render::render_asset::RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}

/// Mouse-drag orbit + wheel zoom + WASD pan (target moves with the camera). Replacement for the
/// full PanOrbitCamera extension (terrain-aware altitude clamp, tilt-shift) landing in Phase 4.
fn camera_input(
    buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut OrbitTarget), With<Camera>>,
) {
    let Ok((mut transform, mut target)) = query.single_mut() else { return };

    // Always drain motion events so they never accumulate between frames.
    let orbiting = buttons.pressed(MouseButton::Right) || buttons.pressed(MouseButton::Middle);
    let mut drag = Vec2::ZERO;
    for event in mouse_motion.read() {
        if orbiting {
            drag += event.delta;
        }
    }

    let mut zoom = 0.0f32;
    for event in wheel.read() {
        zoom += event.y;
    }

    let dt = time.delta_secs();
    let distance_now = (transform.translation - target.0).length();
    let pan_speed = (distance_now * 0.6).max(40.0) * dt;
    let flat_forward = Vec3::new(target.0.x - transform.translation.x, 0.0, target.0.z - transform.translation.z).normalize_or_zero();
    let right = flat_forward.cross(Vec3::Y).normalize_or_zero();
    let mut pan = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) { pan += flat_forward; }
    if keys.pressed(KeyCode::KeyS) { pan -= flat_forward; }
    if keys.pressed(KeyCode::KeyD) { pan += right; }
    if keys.pressed(KeyCode::KeyA) { pan -= right; }
    if pan != Vec3::ZERO {
        target.0 += pan.normalize() * pan_speed;
    }

    // Orbit: azimuth from drag.x, altitude from drag.y; wheel zooms the whole offset.
    let mut offset = transform.translation - target.0;
    offset = Quat::from_rotation_y(-drag.x * 0.008) * offset;
    offset.y = (offset.y - drag.y * 2.0).clamp(18.0, 900.0);
    let scaled = (offset.length() - zoom * 30.0).clamp(60.0, 1400.0);
    transform.translation = target.0 + offset.normalize_or_zero() * scaled;
    transform.look_at(target.0, Vec3::Y);
}

fn log_clock_progress(
    clock: Res<SimulationClock>,
    stats: Res<BuildStats>,
    budget: Res<MunicipalBudget>,
    mut last_day: Local<u32>,
) {
    if clock.day != *last_day {
        *last_day = clock.day;
        info!(
            "Day {} — tick {} — treasury {} cents — buildings placed {}",
            clock.day, clock.tick.0, budget.treasury_cents, stats.buildings_placed
        );
    }
}

fn auto_exit(mut counter: ResMut<FrameCounter>) {
    if let Ok(n) = std::env::var("CFL_EXIT_FRAMES") {
        if let Ok(limit) = n.parse::<u32>() {
            counter.0 += 1;
            if counter.0 >= limit {
                info!("CFL_EXIT_FRAMES reached ({limit}) — exiting");
                std::process::exit(0);
            }
        }
    }
}
