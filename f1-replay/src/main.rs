mod hud;
mod leaderboard;
mod load;
mod playback;
mod race;
mod scene;
mod theme;

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use crate::scene::mesh::build_ribbon_mesh;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiStartupSet, egui};
use f1_data::RawOffset;

use crate::load::SessionBundle;

const SESSION_PATH: &str = "/2023/2023-07-30_Belgian_Grand_Prix/2023-07-30_Race/";

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum AppPhase {
    #[default]
    Loading,
    Running,
}

#[derive(Resource)]
struct LoadRx(Mutex<Receiver<anyhow::Result<SessionBundle>>>);

fn main() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

        let result = rt.block_on(load::load_bundle(SESSION_PATH));

        let _ = tx.send(result);
    });

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
        .insert_resource(LoadRx(Mutex::new(rx)))
        .add_systems(
            PreStartup,
            setup_camera.before(EguiStartupSet::InitContexts),
        )
        .add_systems(Startup, debug_startup)
        .add_systems(Startup, setup_fonts)
        .add_systems(
            Update,
            (poll_load, maybe_enter_running).run_if(in_state(AppPhase::Loading)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (
                splash.run_if(in_state(AppPhase::Loading)),
                debug_transport.run_if(in_state(AppPhase::Running)),
            ),
        )
        .add_systems(
            OnEnter(AppPhase::Running),
            (announce, spawn_track, spawn_cars, fit_camera),
        )
        .insert_resource(playback::PlaybackClock::default())
        .add_systems(
            Update,
            (tick_clock, update_cars)
                .chain()
                .run_if(in_state(AppPhase::Running)),
        )
        .insert_resource(playback::PlaybackClock::starting_at(3600))
        .run();
}

fn tick_clock(mut clock: ResMut<playback::PlaybackClock>) {
    clock.tick(Instant::now());
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
        ));
    }
}

#[derive(Component)]
struct CarMarker {
    num: u8,
    mat: Handle<ColorMaterial>,
    last_live: bool,
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

fn announce(mut bundle: ResMut<SessionBundle>) {
    info!(
        "ready: {} · {} · span {} · {} drivers · centerline {} pts · lap {:.0} m",
        bundle.info.meeting_name,
        bundle.info.session_name,
        bundle.player.duration(),
        bundle.drivers.len(),
        bundle.track.centerline.len(),
        bundle.track.lap_len_m,
    );
    let t = 3600.0;
    let frame = bundle
        .player
        .frame_at(RawOffset(std::time::Duration::from_secs_f64(t)));
    let rows = race::compute_leaderboard(
        &frame.cars,
        &bundle.cl,
        &bundle.crossings,
        t,
        bundle.race_start,
    );
    for (i, r) in rows.iter().take(5).enumerate() {
        let code = bundle
            .drivers
            .get(&r.num)
            .map(|d| d.code.clone())
            .unwrap_or_else(|| "?".into());
        info!("P{} {} lap {} {:?}", i + 1, code, r.lap, r.gap);
    }
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

fn spawn_track(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    bundle: Option<Res<SessionBundle>>,
) {
    let Some(bundle) = bundle else {
        return;
    };
    let frame = &bundle.track;

    let mesh = build_ribbon_mesh(&frame.centerline, 8.0, true);
    let mesh_handle = meshes.add(mesh);

    let mat_handle = materials.add(ColorMaterial::from(theme::chrome_bevy::PANEL));

    commands.spawn((
        Mesh2d(mesh_handle),
        MeshMaterial2d(mat_handle),
        Transform::default(),
        Name::new("TrackCenterline"),
    ));

    let start_pt = frame.centerline[frame.start_idx];
    let tick_mesh = meshes.add(Rectangle::new(4.0, 40.0));

    let tick_mat = materials.add(ColorMaterial::from(theme::chrome_bevy::MUTED));

    commands.spawn((
        Mesh2d(tick_mesh),
        MeshMaterial2d(tick_mat),
        Transform::from_xyz(start_pt.x, start_pt.y, 0.0),
        Name::new("StartFinish"),
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
const MIN_SPLASH_SECS: f32 = 1.5;

#[derive(Resource, Default)]
struct PendingBundle(Option<SessionBundle>);

fn poll_load(rx: Res<LoadRx>, mut pending: ResMut<PendingBundle>) {
    if pending.0.is_none() {
        if let Ok(result) = rx.0.lock().unwrap().try_recv() {
            match result {
                Ok(bundle) => pending.0 = Some(bundle),
                Err(e) => {
                    eprintln!("load failed: {e:?}");
                    std::process::exit(1);
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
) {
    if time.elapsed_secs() < MIN_SPLASH_SECS || pending.0.is_none() {
        return;
    }
    if let Some(bundle) = pending.0.take() {
        commands.insert_resource(bundle);
        next.set(AppPhase::Running);
    }
}

fn splash(mut contexts: EguiContexts, time: Res<Time>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

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

fn debug_startup() {
    println!("[bevy] Startup reached");
}

#[derive(Default)]
struct FpsAcc {
    frames: u32,
    window_start: f32,
    value: f32,
}

fn debug_transport(
    mut contexts: EguiContexts,
    mut clock: ResMut<playback::PlaybackClock>,
    mut bundle: Option<ResMut<SessionBundle>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut acc: Local<FpsAcc>,
) {
    acc.frames += 1;
    let now = time.elapsed_secs();
    if now - acc.window_start >= 1.0 {
        acc.value = acc.frames as f32 / (now - acc.window_start);
        acc.frames = 0;
        acc.window_start = now;
    }
    let fps = acc.value;

    let (Some(bundle), Ok(ctx)) = (bundle.as_mut(), contexts.ctx_mut()) else {
        return;
    };
    let max_secs = bundle.player.duration().0.as_secs_f32();
    let frame = bundle.player.frame_at(RawOffset(clock.t));

    let mut root_ui = egui::Ui::new(
        ctx.clone(),
        "main_ui".into(),
        egui::UiBuilder::new().max_rect(ctx.viewport_rect()),
    );

    // HUD top bar
    hud::render_hud(&mut root_ui, &bundle.info, &frame, clock.t);

    let rows = race::compute_leaderboard(
        &frame.cars,
        &bundle.cl,
        &bundle.crossings,
        clock.t.as_secs_f64(),
        bundle.race_start,
    );

    leaderboard::render_leaderboard(&mut root_ui, &rows, &bundle.drivers, &bundle.palette);

    // Transport bar (existing)
    egui::Panel::bottom("transport")
        .frame(egui::Frame::NONE.fill(egui::Color32::from_black_alpha(140)))
        .show(&mut root_ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(if clock.playing { "||" } else { ">" }).clicked() {
                    clock.playing = !clock.playing;
                }
                for s in [0.5f32, 1.0, 2.0, 4.0, 8.0] {
                    if ui
                        .selectable_label(clock.speed == s, format!("{s}x"))
                        .clicked()
                    {
                        clock.speed = s;
                    }
                }
                let mut secs = clock.t.as_secs_f32();
                let resp = ui.add(egui::Slider::new(&mut secs, 0.0..=max_secs).show_value(false));
                if resp.changed() {
                    clock.t = Duration::from_secs_f32(secs);
                }
                ui.monospace(format!(
                    "{:02}:{:02} / {:02}:{:02}",
                    secs as u32 / 60,
                    secs as u32 % 60,
                    max_secs as u32 / 60,
                    max_secs as u32 % 60
                ));
                ui.monospace(format!("{fps:.0} fps"));
            });
        });

    if keys.just_pressed(KeyCode::Space) {
        clock.playing = !clock.playing;
    }
}
