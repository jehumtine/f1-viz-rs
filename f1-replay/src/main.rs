mod hud;
mod leaderboard;
mod load;
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

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiStartupSet, egui};
use f1_data::RawOffset;

use crate::load::SessionBundle;
use crate::scene::mesh::build_ribbon_mesh;
use crate::session_index::{IndexChannel, IndexStatus, SessionEntry, SessionIndex, SessionKind};

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
        .add_systems(Update, tick_clock)
        .add_systems(Update, update_cars.run_if(in_state(AppPhase::Running)))
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
    commands.spawn((Camera2d, Transform::from_xyz(0.0, 0.0, 100.0)));
}

#[derive(Component)]
struct CarMarker {
    num: u8,
    mat: Handle<ColorMaterial>,
    last_live: bool,
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
        let bevy_color = Color::srgba_u8(c.r(), c.g(), c.b(), c.a());
        let mat_handle = materials.add(ColorMaterial::from(bevy_color));

        commands.spawn((
            Mesh2d(circle.clone()),
            MeshMaterial2d(mat_handle.clone()),
            Transform::from_xyz(0.0, 0.0, 1.0),
            Name::new(format!("Car #{num}")),
            CarMarker {
                num,
                mat: mat_handle,
                last_live: true,
            },
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
) {
    let (Some(bundle), Ok(window)) = (bundle, windows.single()) else {
        return;
    };
    let frame = &bundle.track;

    let size = frame.size();
    let scale_x = size.x / window.width();
    let scale_y = size.y / window.height();
    let scale = scale_x.max(scale_y) / 0.92;

    for (mut transform, mut proj) in query.iter_mut() {
        transform.translation.x = frame.center().x;
        transform.translation.y = frame.center().y;
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

    let mut root_ui = egui::Ui::new(
        ctx.clone(),
        "main_ui".into(),
        egui::UiBuilder::new().max_rect(ctx.viewport_rect()),
    );

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
