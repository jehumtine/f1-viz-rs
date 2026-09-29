mod load;
mod scene;
mod theme;

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};

use crate::scene::mesh::build_ribbon_mesh;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiStartupSet, egui};

use crate::load::SessionBundle;

const SESSION_PATH: &str = "/2023/2023-05-07_Miami_Grand_Prix/2023-05-07_Race/";

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
        println!("[load] thread started");

        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

        println!("[load] tokio runtime created");

        let result = rt.block_on(load::load_bundle(SESSION_PATH));

        println!("[load] bundle load finished: {}", result.is_ok());

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
        .init_state::<AppPhase>()
        .insert_resource(LoadRx(Mutex::new(rx)))
        .add_systems(
            PreStartup,
            setup_camera.before(EguiStartupSet::InitContexts),
        )
        .add_systems(Startup, debug_startup)
        .add_systems(Startup, setup_fonts)
        .add_systems(Update, poll_load.run_if(in_state(AppPhase::Loading)))
        .add_systems(
            EguiPrimaryContextPass,
            splash.run_if(in_state(AppPhase::Loading)),
        )
        .run();
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

fn poll_load(rx: Res<LoadRx>, mut commands: Commands, mut next: ResMut<NextState<AppPhase>>) {
    if let Ok(result) = rx.0.lock().unwrap().try_recv() {
        match result {
            Ok(bundle) => {
                commands.insert_resource(bundle);
                next.set(AppPhase::Running);
            }
            Err(e) => {
                eprintln!("load failed: {e:?}");
                std::process::exit(1);
            }
        }
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
