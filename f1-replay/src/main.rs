mod hud;
mod leaderboard;
mod load;
mod overlay;
mod playback;
mod race;
mod scene;
mod session_index;
mod telemetry;
mod theme;
mod transport;

use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bevy::camera::CameraOutputMode;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseWheel;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiStartupSet, egui};
use f1_data::RawOffset;

use crate::load::SessionBundle;
use crate::scene::mesh::build_ribbon_mesh;
use crate::session_index::{IndexChannel, IndexStatus, SessionEntry, SessionIndex};

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum AppPhase {
    #[default]
    Browse,
    Loading,
    Running,
}

#[derive(Resource)]
struct LoadRx(Mutex<mpsc::Receiver<anyhow::Result<SessionBundle>>>);

#[derive(Resource, Default)]
struct CurrentSessionPath(String);

#[derive(Component)]
struct SessionEntity;

#[derive(Resource, Default)]
struct PendingBundle(Option<SessionBundle>);

const MIN_SPLASH_SECS: f32 = 1.5;

#[derive(Default)]
struct FpsAcc {
    frames: u32,
    window_start: f32,
    value: f32,
}

#[derive(Resource)]
struct CameraTarget {
    position: Vec2,
    scale: f32,
}

#[derive(Resource, Default)]
struct FlagCrossfade {
    previous: Option<String>,
    alpha: f32,
    duration: f32,
}

fn main() {
    // Start with a dummy load channel; replaced on first real session change.
    let (_, load_rx) = mpsc::channel::<anyhow::Result<SessionBundle>>();

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "f1-replay".into(),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(ClearColor(theme::chrome_bevy::ASPHALT))
        .insert_resource(PendingBundle::default())
        .init_state::<AppPhase>()
        .insert_resource(CurrentSessionPath::default())
        .insert_resource(SessionIndex::default())
        .insert_resource(IndexChannel::default())
        .insert_resource(LoadRx(Mutex::new(load_rx)))
        .insert_resource(playback::PlaybackClock::default())
        .insert_resource(telemetry::TelemetrySelection::default())
        .insert_resource(CameraRig::default())
        .insert_resource(overlay::TrailState::default())
        .insert_resource(CameraTarget {
            position: Vec2::ZERO,
            scale: 1.0,
        })
        .insert_resource(FlagCrossfade::default())
        .add_systems(
            PreStartup,
            setup_camera.before(EguiStartupSet::InitContexts),
        )
        .add_systems(Startup, (debug_startup, setup_fonts))
        // Phase transitions
        .add_systems(
            OnEnter(AppPhase::Browse),
            session_index::request_years_on_enter,
        )
        .add_systems(Update, session_index::poll_index)
        .add_systems(
            OnEnter(AppPhase::Loading),
            (clear_session_entities, clear_bundle, on_session_change),
        )
        .add_systems(
            Update,
            (poll_load, maybe_enter_running).run_if(in_state(AppPhase::Loading)),
        )
        .add_systems(
            OnEnter(AppPhase::Running),
            (announce, spawn_track, spawn_cars, fit_camera),
        )
        // UI
        .add_systems(
            EguiPrimaryContextPass,
            (
                browse_ui.run_if(in_state(AppPhase::Browse)),
                splash.run_if(in_state(AppPhase::Loading)),
                render_ui.run_if(in_state(AppPhase::Running)),
            ),
        )
        // Simulation
        .add_systems(
            Update,
            (tick_clock, update_cars, highlight_cars, camera_control)
                .chain()
                .run_if(in_state(AppPhase::Running)),
        )
        .run();
}

fn tick_clock(mut clock: ResMut<playback::PlaybackClock>) {
    clock.tick(Instant::now());
}

fn debug_startup() {
    println!("[bevy] Startup reached");
}

fn setup_fonts(mut contexts: EguiContexts) -> Result {
    println!("[egui] setup_fonts entered");
    let ctx = contexts.ctx_mut()?;
    println!("[egui] got primary context");
    theme::FontRoles::install(ctx);
    println!("[egui] fonts installed");
    Ok(())
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Transform::from_xyz(0.0, 0.0, 100.0),
        Tonemapping::None,
        Bloom {
            intensity: 0.4,
            ..default()
        },
    ));
}

#[derive(Component)]
struct CarMarker {
    num: u8,
    mat: Handle<ColorMaterial>,
    base: LinearRgba,
}

fn spawn_cars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    bundle: Option<Res<SessionBundle>>,
) {
    let Some(bundle) = bundle else { return };
    let circle = meshes.add(Circle::new(6.0));

    for (&num, _driver) in &bundle.drivers {
        let c = bundle.palette.get(num);
        let srgb = Color::srgba_u8(c.r(), c.g(), c.b(), c.a());
        let base = LinearRgba::from(srgb);
        let bevy_color = Color::srgba_u8(c.r(), c.g(), c.b(), c.a());
        let mat = materials.add(ColorMaterial::from(bevy_color));

        commands.spawn((
            Mesh2d(circle.clone()),
            MeshMaterial2d(mat.clone()),
            Transform::from_xyz(0.0, 0.0, 1.0),
            Name::new(format!("Car #{num}")),
            CarMarker { num, mat, base },
            SessionEntity,
        ));
    }
}

fn update_cars(
    clock: Res<playback::PlaybackClock>,
    mut bundle: Option<ResMut<SessionBundle>>,
    mut query: Query<(&CarMarker, &mut Transform)>,
) {
    let Some(bundle) = bundle.as_mut() else {
        return;
    };
    let frame = bundle.player.frame_at(RawOffset(clock.t));

    for (marker, mut xform) in query.iter_mut() {
        if let Some(state) = frame.cars.get(&marker.num) {
            xform.translation.x = state.position.x_m as f32;
            xform.translation.y = state.position.y_m as f32;
        }
    }
}

fn announce(mut bundle: ResMut<SessionBundle>, clock: Res<playback::PlaybackClock>) {
    info!(
        "ready: {} · {} · span {} · {} drivers · centerline {} pts · lap {:.0} m",
        bundle.info.meeting_name,
        bundle.info.session_name,
        bundle.player.duration(),
        bundle.drivers.len(),
        bundle.track.centerline.len(),
        bundle.track.lap_len_m,
    );

    let t = clock.t.as_secs_f64();
    let frame = bundle
        .player
        .frame_at(RawOffset(std::time::Duration::from_secs_f64(t)));
    let rows = bundle.race.classify(t, &frame.cars);
    for (i, r) in rows.iter().take(5).enumerate() {
        let code = bundle
            .drivers
            .get(&r.num)
            .map(|d| d.code.clone())
            .unwrap_or_else(|| "?".into());
        info!("P{} {} lap {} {:?}", i + 1, code, r.lap, r.gap);
    }
}

fn spawn_track(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    bundle: Option<Res<SessionBundle>>,
) {
    let Some(bundle) = bundle else { return };
    let frame = &bundle.track;

    let mesh = build_ribbon_mesh(&frame.centerline, 8.0, true);
    let mesh_handle = meshes.add(mesh);
    let mat_handle = materials.add(ColorMaterial::from(theme::chrome_bevy::PANEL));

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(mat_handle),
        Transform::default(),
        Name::new("TrackCenterline"),
        SessionEntity,
    ));

    let start_pt = frame.centerline[frame.start_idx];
    let tick_mesh = meshes.add(Rectangle::new(4.0, 40.0));
    let tick_mat = materials.add(ColorMaterial::from(theme::chrome_bevy::MUTED));

    commands.spawn((
        Mesh2d(tick_mesh),
        MeshMaterial2d(tick_mat),
        Transform::from_xyz(start_pt.x, start_pt.y, 0.0),
        Name::new("StartFinish"),
        SessionEntity,
    ));
}

fn fit_camera(
    bundle: Option<Res<SessionBundle>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<(&mut Transform, &mut Projection), With<Camera>>,
    mut rig: ResMut<CameraRig>,
) {
    let (Some(bundle), Ok(window)) = (bundle, windows.single()) else {
        return;
    };
    let frame = &bundle.track;

    let size = frame.size();
    let scale = (size.x / window.width()).max(size.y / window.height()) / 0.92;

    rig.fit_scale = scale;
    rig.center = Vec2::new(frame.center().x, frame.center().y);
    rig.follow = None;

    for (mut transform, mut proj) in query.iter_mut() {
        transform.translation.x = rig.center.x;
        transform.translation.y = rig.center.y;
        transform.translation.z = 100.0;
        if let Projection::Orthographic(ortho) = proj.as_mut() {
            ortho.scale = scale;
        }
    }
}

fn clear_bundle(mut commands: Commands) {
    commands.remove_resource::<SessionBundle>();
}

fn clear_session_entities(mut commands: Commands, doomed: Query<Entity, With<SessionEntity>>) {
    for e in doomed.iter() {
        commands.entity(e).despawn();
    }
}

fn on_session_change(current_path: Res<CurrentSessionPath>, mut commands: Commands) {
    let path = current_path.0.clone();
    if path.is_empty() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        let result = rt.block_on(load::load_bundle(&path));
        let _ = tx.send(result);
    });
    commands.insert_resource(LoadRx(Mutex::new(rx)));
}

fn poll_load(rx: Res<LoadRx>, mut pending: ResMut<PendingBundle>) {
    if pending.0.is_none() {
        if let Ok(result) = rx.0.lock().unwrap().try_recv() {
            match result {
                Ok(bundle) => {
                    pending.0 = Some(bundle);
                }
                Err(e) => {
                    eprintln!("load failed: {e:?}");
                    // Don't exit; fall back to Browse so user can try another.
                    pending.0 = None;
                }
            }
        }
    }
}

fn maybe_enter_running(
    mut pending: ResMut<PendingBundle>,
    time: Res<Time>,
    mut commands: Commands,
    mut next: ResMut<NextState<AppPhase>>,
    mut clock: ResMut<playback::PlaybackClock>,
    mut trails: ResMut<overlay::TrailState>,
) {
    if time.elapsed_secs() < MIN_SPLASH_SECS || pending.0.is_none() {
        return;
    }
    if let Some(bundle) = pending.0.take() {
        // Reset clock for the new session.
        clock.t = Duration::from_secs(3600);
        clock.playing = true;
        clock.speed = 1.0;
        commands.insert_resource(bundle);
        next.set(AppPhase::Running);
        trails.trails.clear();
    }
}

fn splash(mut contexts: EguiContexts, time: Res<Time>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut root_ui = egui::Ui::new(
        ctx.clone(),
        "splash_root".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::chrome::ASPHALT))
        .show(&mut root_ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.42);

                ui.label(
                    egui::RichText::new("F1 REPLAY")
                        .font(theme::FontRoles::display(44.0))
                        .color(theme::chrome::TEXT),
                );

                let dots = ".".repeat((time.elapsed_secs() as usize % 3) + 1);

                ui.label(
                    egui::RichText::new(format!("loading session{dots:<3}"))
                        .font(theme::FontRoles::mono(14.0))
                        .color(theme::chrome::MUTED),
                );
            });
        });
}

#[derive(SystemParam)]
struct PickerParams<'w> {
    index: ResMut<'w, SessionIndex>,
    channel: Res<'w, IndexChannel>,
    current: ResMut<'w, CurrentSessionPath>,
    next: ResMut<'w, NextState<AppPhase>>,
}

fn render_picker(ui: &mut egui::Ui, p: &mut PickerParams, expanded: &mut HashSet<u32>, max_h: f32) {
    match &p.index.years_status {
        IndexStatus::Idle | IndexStatus::Fetching => {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("contacting archive…")
                        .font(theme::FontRoles::mono(16.0))
                        .color(theme::chrome::MUTED),
                );
            });
            ui.add_space(20.0);
            return;
        }
        IndexStatus::Failed(e) => {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("archive unreachable")
                        .font(theme::FontRoles::display(18.0))
                        .color(theme::chrome::TEXT),
                );
                ui.label(
                    egui::RichText::new(e.clone())
                        .font(theme::FontRoles::mono(12.0))
                        .color(theme::chrome::MUTED),
                );
            });
            ui.add_space(20.0);
            return;
        }
        IndexStatus::Ready => {}
    }

    egui::ScrollArea::vertical()
        .max_height(max_h)
        .auto_shrink(false)
        .show(ui, |ui| {
            let years: Vec<u32> = p.index.years.clone();
            for year in years {
                let is_open = expanded.contains(&year);
                let loaded = p.index.loaded_years.contains(&year);
                let fetching = p.index.fetching_years.contains(&year);

                // ---- Year row: full-width hoverable bar ----
                let (label, color) = if fetching {
                    (format!("…  {year}"), theme::chrome::MUTED)
                } else if let Some(msg) = p.index.failed_years.get(&year) {
                    (format!("x  {year}\n     {msg}"), theme::chrome::MUTED)
                } else if is_open {
                    (format!("−  {year}"), theme::chrome::AMBER)
                } else {
                    (format!("+  {year}"), theme::chrome::TEXT)
                };

                let available = ui.available_width();
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(available, 34.0), egui::Sense::click());
                if resp.hovered() && !is_open {
                    ui.painter().rect_filled(
                        rect.expand2(egui::vec2(4.0, 0.0)),
                        6.0,
                        egui::Color32::from_white_alpha(12),
                    );
                }
                ui.painter().text(
                    rect.left_center() + egui::vec2(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    label,
                    theme::FontRoles::display(20.0),
                    color,
                );
                if resp.clicked() {
                    if is_open {
                        expanded.remove(&year);
                    } else {
                        expanded.insert(year);
                        session_index::request_year(&mut p.index, &p.channel.sender, year);
                    }
                }

                // ---- Expanded: meetings + session chips ----
                if is_open && loaded {
                    let mut groups: Vec<(u32, Vec<SessionEntry>)> = Vec::new();
                    for e in p.index.entries.iter().filter(|e| e.year == year) {
                        match groups.last_mut() {
                            Some((r, v)) if *r == e.round => v.push(e.clone()),
                            _ => groups.push((e.round, vec![e.clone()])),
                        }
                    }
                    for (_, group) in groups {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(&group[0].meeting)
                                    .font(theme::FontRoles::body(16.0))
                                    .color(theme::chrome::TEXT),
                            );
                        });
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(12.0);
                            for e in &group {
                                let active = p.current.0 == e.path;
                                let text = egui::RichText::new(e.kind.label())
                                    .font(theme::FontRoles::body(14.0))
                                    .color(if active {
                                        theme::chrome::AMBER
                                    } else {
                                        theme::chrome::MUTED
                                    });
                                let desired = ui.spacing().interact_size;
                                let (rect, resp) = ui.allocate_at_least(
                                    egui::vec2(56.0, 26.0),
                                    egui::Sense::click(),
                                );
                                let bg = if active {
                                    egui::Color32::from_rgba_unmultiplied(0xFF, 0xB1, 0x00, 40)
                                } else if resp.hovered() {
                                    egui::Color32::from_white_alpha(20)
                                } else {
                                    egui::Color32::from_white_alpha(8)
                                };
                                ui.painter().rect_filled(rect, 6.0, bg);
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    e.kind.label(),
                                    theme::FontRoles::body(14.0),
                                    if active {
                                        theme::chrome::AMBER
                                    } else if resp.hovered() {
                                        theme::chrome::TEXT
                                    } else {
                                        theme::chrome::MUTED
                                    },
                                );
                                let _ = (text, desired); // silence unused
                                if resp.clicked() {
                                    p.current.0 = e.path.clone();
                                    p.next.set(AppPhase::Loading);
                                }
                                ui.add_space(6.0);
                            }
                        });
                        ui.add_space(6.0);
                    }
                } else if is_open && fetching {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        ui.label(
                            egui::RichText::new("loading meetings…")
                                .font(theme::FontRoles::mono(13.0))
                                .color(theme::chrome::MUTED),
                        );
                    });
                    ui.add_space(6.0);
                }
            }
        });
}

fn browse_ui(mut contexts: EguiContexts, mut p: PickerParams, mut expanded: Local<HashSet<u32>>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.viewport_rect();

    egui::Area::new("browse".into())
        .pivot(egui::Align2::CENTER_CENTER)
        .fixed_pos(screen.center())
        .show(ctx, |ui| {
            ui.set_width(620.0);
            theme::glass().show(ui, |ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("F1 REPLAY")
                            .font(theme::FontRoles::display(48.0))
                            .color(theme::chrome::TEXT),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("select a session to begin")
                            .font(theme::FontRoles::mono(15.0))
                            .color(theme::chrome::MUTED),
                    );
                });
                ui.add_space(20.0);
                ui.separator();
                ui.add_space(12.0);
                render_picker(ui, &mut p, &mut expanded, 460.0);
                ui.add_space(8.0);
            });
        });
}

#[derive(SystemParam)]
struct UiState<'w, 's> {
    clock: ResMut<'w, playback::PlaybackClock>,
    bundle: Option<ResMut<'w, SessionBundle>>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    time: Res<'w, Time>,
    acc: Local<'s, FpsAcc>,
    telemetry_sel: ResMut<'w, telemetry::TelemetrySelection>,
    cam: Query<'w, 's, (&'static Transform, &'static Projection), With<Camera2d>>,
    trails: ResMut<'w, overlay::TrailState>,
}

fn render_ui(
    mut contexts: EguiContexts,
    mut state: UiState,
    mut p: PickerParams,
    mut picker_open: Local<bool>,
    mut expanded: Local<HashSet<u32>>,
) {
    state.acc.frames += 1;
    let now = state.time.elapsed_secs();
    if now - state.acc.window_start >= 1.0 {
        state.acc.value = state.acc.frames as f32 / (now - state.acc.window_start);
        state.acc.frames = 0;
        state.acc.window_start = now;
    }
    let fps = state.acc.value;

    let (Some(bundle), Ok(ctx)) = (state.bundle.as_mut(), contexts.ctx_mut()) else {
        return;
    };
    let max_secs = bundle.player.duration().0.as_secs_f32();
    let frame = bundle.player.frame_at(RawOffset(state.clock.t));
    let rows = bundle
        .race
        .classify(state.clock.t.as_secs_f64(), &frame.cars);

    let mut trail_nums: Vec<u8> = state.telemetry_sel.0.clone();
    if let Some(r) = rows.first() {
        if !trail_nums.contains(&r.num) {
            trail_nums.push(r.num);
        }
    }
    overlay::update_trails(&mut state.trails.deref_mut(), &trail_nums, &frame.cars);

    let mut root_ui = egui::Ui::new(
        ctx.clone(),
        "main_ui".into(),
        egui::UiBuilder::new().max_rect(ctx.viewport_rect()),
    );
    if let Ok((cam_t, proj)) = state.cam.single() {
        if let Projection::Orthographic(ortho) = proj {
            let cam = cam_t.translation.truncate();
            let screen = ctx.viewport_rect();
            overlay::render_trails(
                ctx,
                screen,
                egui::vec2(cam.x, cam.y),
                ortho.scale,
                &state.trails.deref_mut(),
                &bundle.palette,
            );

            overlay::render_car_labels(
                ctx,
                screen,
                egui::vec2(cam.x, cam.y),
                ortho.scale,
                &frame.cars,
                &bundle.drivers,
                &state.telemetry_sel.deref_mut(),
            );
        }
    }

    hud::render_hud(&mut root_ui, &bundle.info, &frame, state.clock.t);

    let rows = bundle
        .race
        .classify(state.clock.t.as_secs_f64(), &frame.cars);
    leaderboard::render_leaderboard(
        &mut root_ui,
        &rows,
        &bundle.drivers,
        &bundle.palette,
        state.telemetry_sel.deref_mut(),
    );
    telemetry::render_panel(
        ctx,
        &frame.cars,
        &bundle.drivers,
        &bundle.palette,
        &state.telemetry_sel.deref_mut(),
    );

    transport::render_transport(
        &mut root_ui,
        &mut state.clock.deref_mut(),
        max_secs,
        fps,
        &state.keys.deref(),
    );

    // Toggleable picker: a small chip top-left to open, the card when open.
    let screen = ctx.viewport_rect();
    if !*picker_open {
        egui::Area::new("picker_chip".into())
            .pivot(egui::Align2::LEFT_TOP)
            .fixed_pos(egui::pos2(screen.min.x + 14.0, screen.min.y + 14.0))
            .show(ctx, |ui| {
                theme::glass().show(ui, |ui| {
                    if ui
                        .add(
                            egui::Label::new(
                                egui::RichText::new("SESSIONS")
                                    .font(theme::FontRoles::display(12.0))
                                    .color(theme::chrome::MUTED),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .clicked()
                    {
                        *picker_open = true;
                    }
                });
            });
    } else {
        egui::Area::new("picker_panel".into())
            .pivot(egui::Align2::LEFT_TOP)
            .fixed_pos(egui::pos2(screen.min.x + 14.0, screen.min.y + 14.0))
            .show(ctx, |ui| {
                ui.set_width(360.0);
                theme::glass().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("SESSIONS")
                                .font(theme::FontRoles::display(14.0))
                                .color(theme::chrome::AMBER),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Label::new(
                                        egui::RichText::new("✕")
                                            .font(theme::FontRoles::body(14.0))
                                            .color(theme::chrome::MUTED),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .clicked()
                            {
                                *picker_open = false;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    render_picker(ui, &mut p, &mut expanded, 380.0);
                });
            });
    }
}

fn highlight_cars(
    sel: Res<telemetry::TelemetrySelection>,
    mut q: Query<(&CarMarker, &mut Transform)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (m, mut t) in q.iter_mut() {
        let selected = sel.0.contains(&m.num);
        t.scale = Vec3::splat(if selected { 1.6 } else { 1.0 });

        if let Some(mut mat) = materials.get_mut(&m.mat) {
            mat.color = if selected {
                Color::LinearRgba(LinearRgba::new(
                    m.base.red * 2.5,
                    m.base.green * 2.5,
                    m.base.blue * 2.5,
                    1.0,
                ))
            } else {
                Color::LinearRgba(m.base)
            };
        }
    }
}

#[derive(Resource, Default)]
struct CameraRig {
    follow: Option<u8>,
    fit_scale: f32,
    center: Vec2,
}

#[derive(Default)]
struct MouseTrack {
    down: bool,
    moved: bool,
    last: Vec2,
    origin: Vec2,
}

fn camera_control(
    wants_input: Res<EguiWantsInput>,
    mut rig: ResMut<CameraRig>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cam: Query<(&mut Transform, &mut Projection), (With<Camera2d>, Without<CarMarker>)>,
    car_xforms: Query<(&CarMarker, &Transform), Without<Camera2d>>,
    mut mt: Local<MouseTrack>,
) {
    let Ok(window) = windows.single() else { return };
    let egui_wants = wants_input.wants_any_pointer_input();

    let Ok((mut xform, mut proj)) = cam.single_mut() else {
        return;
    };
    let Projection::Orthographic(ortho) = proj.as_mut() else {
        return;
    };
    let scale = ortho.scale;
    let center_px = Vec2::new(window.width() / 2.0, window.height() / 2.0);
    let cursor = window.cursor_position().map(|c| Vec2::new(c.x, c.y));

    // ---- wheel: zoom to cursor ----
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy != 0.0 && !egui_wants {
        let factor = 1.1f32.powf(-dy);
        let new_scale = (scale * factor).clamp(rig.fit_scale * 0.1, rig.fit_scale * 15.0);
        if let Some(c) = cursor {
            let cursor_world =
                xform.translation.truncate() + (c - center_px) * Vec2::new(scale, -scale);
            let k = new_scale / scale;
            xform.translation.x = cursor_world.x - (cursor_world.x - xform.translation.x) * k;
            xform.translation.y = cursor_world.y - (cursor_world.y - xform.translation.y) * k;
        }
        ortho.scale = new_scale;
        rig.follow = None;
    }

    // ---- left button: drag = pan, clean click = follow/unfollow ----
    if let Some(c) = cursor {
        if mouse.just_pressed(MouseButton::Left) && !egui_wants {
            mt.down = true;
            mt.moved = false;
            mt.origin = c;
            mt.last = c;
        }
        if mouse.pressed(MouseButton::Left) && mt.down {
            let delta = c - mt.last;
            if (c - mt.origin).length() > 5.0 {
                mt.moved = true;
            }
            if mt.moved && !egui_wants {
                xform.translation.x -= delta.x * scale;
                xform.translation.y += delta.y * scale;
                rig.follow = None;
            }
            mt.last = c;
        }
        if mouse.just_released(MouseButton::Left) {
            if mt.down && !mt.moved && !egui_wants {
                let cursor_world =
                    xform.translation.truncate() + (c - center_px) * Vec2::new(scale, -scale);
                let hit = car_xforms
                    .iter()
                    .filter(|(_, t)| {
                        (Vec2::new(t.translation.x, t.translation.y) - cursor_world).length()
                            < 12.0 * scale
                    })
                    .min_by(|(_, a), (_, b)| {
                        let da = (Vec2::new(a.translation.x, a.translation.y) - cursor_world)
                            .length_squared();
                        let db = (Vec2::new(b.translation.x, b.translation.y) - cursor_world)
                            .length_squared();
                        da.partial_cmp(&db).unwrap()
                    })
                    .map(|(m, _)| m.num);
                rig.follow = hit;
            }
            mt.down = false;
        }
    }

    // ---- keys ----
    if keys.just_pressed(KeyCode::Escape) {
        rig.follow = None;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        xform.translation.x = rig.center.x;
        xform.translation.y = rig.center.y;
        ortho.scale = rig.fit_scale;
        rig.follow = None;
    }

    // ---- follow mode: glide after the car ----
    if let Some(num) = rig.follow {
        if let Some((_, t)) = car_xforms.iter().find(|(m, _)| m.num == num) {
            let target = Vec2::new(t.translation.x, t.translation.y);
            let a = 1.0 - (-8.0 * time.delta_secs()).exp();
            xform.translation.x += (target.x - xform.translation.x) * a;
            xform.translation.y += (target.y - xform.translation.y) * a;
        }
    }
}
