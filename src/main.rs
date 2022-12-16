#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use core::time::Duration;
use eframe::egui;
use livesplit_core::rendering::software;
use livesplit_core::run::parser::composite;
use livesplit_core::run::saver::livesplit;
use livesplit_core::{Layout, Run, Segment, SharedTimer, Timer};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
//use livesplit_core::{HotkeyConfig, HotkeySystem, Layout, Run, Segment, SharedTimer, Timer};
//use livesplit_hotkey;

fn main() {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(350.0, 500.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Livesplit-core / egui",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    );
}

const SPLIT_SAVE: &'static str = "boko_timer_splits.lss";

type Callback = fn(&mut MyApp);
struct MyApp {
    stimer: SharedTimer,
    layout: Layout,
    renderer: software::Renderer,
    keys: Vec<egui::Key>,
    func: HashMap<egui::Key, Callback>,
    frame: egui::Frame,
}

fn read_file(path: PathBuf) -> Option<Run> {
    let file = File::open(&path).ok()?;
    let file = BufReader::new(file);
    let load_files = true;
    // Actually parse the file.
    let result = composite::parse(file, Some(path), load_files);
    let parsed = result.expect("Not a valid splits file");
    Some(parsed.run)
}

impl MyApp {
    fn split(app: &mut MyApp) {
        dbg!("split");
        app.stimer.write().split_or_start();
        app.save_state();
    }
    fn save_state(&mut self) {
        self.save_file(&PathBuf::from(SPLIT_SAVE));
    }
    fn save_file(&mut self, filename: &PathBuf) {
        let timer = self.stimer.read();
        let snapshot = timer.snapshot();
        let file = File::create(filename);
        let writer = BufWriter::new(file.expect("Failed creating the file"));
        livesplit::save_timer(&snapshot, writer).expect("Couldn't save the splits file");
    }
    fn undo_split(app: &mut MyApp) {
        app.stimer.write().undo_split();
        app.save_state();
    }
    fn skip_split(app: &mut MyApp) {
        app.stimer.write().skip_split();
        app.save_state();
    }
    fn reset(app: &mut MyApp) {
        app.stimer.write().reset(true);
    }
    fn pause(app: &mut MyApp) {
        app.stimer.write().toggle_pause();
        app.save_state();
    }
    fn save(app: &mut MyApp) {
        dbg!("save");
        app.stimer.write().reset(true);
        let mut path = app.stimer.read().run().path().clone();
        if path.is_none() {
            path = rfd::FileDialog::new()
                .add_filter("Livesplit", &["lss"])
                .save_file()
        }
        if let Some(filename) = path {
            app.save_file(&filename);
        }
    }
    fn open(app: &mut MyApp) {
        println!("open");
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            dbg!(&path);
            if let Some(run) = read_file(path) {
                app.stimer
                    .write()
                    .replace_run(run, false)
                    .expect("Run has no segments, sus");
            }
        } else {
            dbg!("no file picked");
        }
    }
    fn comparison(app: &mut MyApp) {
        app.stimer.write().switch_to_next_comparison();
        dbg!(app.stimer.read().current_comparison());
    }
    fn hide(app: &mut MyApp) {
        app.stimer
            .write()
            .set_current_comparison("None")
            .expect("Could not set to None");
    }
    fn hotkey(&mut self, ctx: &egui::Context) {
        let keys: Vec<_> = self
            .keys
            .iter()
            .filter(|key| ctx.input().key_pressed(**key))
            .cloned()
            .collect();
        for key in keys {
            if let Some(fun) = self.func.get(&key) {
                fun(self);
            }
        }
    }
    fn render(&mut self, frame: &eframe::Frame) -> egui::ImageData {
        let timer = self.stimer.read();

        let snapshot = timer.snapshot();
        let layout_state = self.layout.state(&snapshot);
        let fsize = frame.info().window_info.size;
        let size = [fsize.x as u32, fsize.y as u32];
        let size2 = [fsize.x as usize, fsize.y as usize];

        self.renderer.render(&layout_state, size);
        let rgba = self.renderer.image_data();
        let image = egui::ImageData::Color(egui::ColorImage::from_rgba_unmultiplied(size2, rgba));
        image
    }
}

impl Default for MyApp {
    fn default() -> Self {
        // Create a run object that we can use with at least one segment.

        let run = if let Some(run) = read_file(PathBuf::from(SPLIT_SAVE)) {
            dbg!("previous run");
            println!("game_name {}", run.game_name());
            println!("category  {}", run.category_name());
            println!("modified  {}", run.has_been_modified());
            println!("last_attampt:");
            let attempts = run.attempt_history();
            let n = attempts.len();
            if n > 0 {
                let last = &attempts[n - 1];
                let index = last.index();
                println!("started: {:?}", last.started());
                println!("ended: {:?}", last.ended());
                let time = last.time();
                if time.real_time.is_none() && time.game_time.is_none() {
                    println!("attempt not finished");
                } else {
                    println!("attempt finished, possibly unsaved");
                }
            }
            run
        } else {
            dbg!("new run");
            let mut run = Run::new();
            run.set_game_name("Breath of the Wild");
            run.set_category_name("100%");
            run.push_segment(Segment::new("Paraglider"));
            run.push_segment(Segment::new("IST"));
            run.push_segment(Segment::new("Vah Medoh"));
            run.push_segment(Segment::new("Ganon"));
            run.push_segment(Segment::new("Korok 900"));
            run
        };

        let stimer = Timer::new(run)
            .expect("Run with at least one segment provided")
            .into_shared();
        let layout = Layout::default_layout();

        let renderer = software::Renderer::new();

        // osx has no support for global hotkeys via livesplit
        //let hotkey = HotkeySystem::new(stimer.clone());

        let mut func: HashMap<egui::Key, Callback> = HashMap::new();
        func.insert(egui::Key::Space, MyApp::split);
        func.insert(egui::Key::P, MyApp::pause);
        func.insert(egui::Key::U, MyApp::undo_split);
        func.insert(egui::Key::S, MyApp::skip_split);
        func.insert(egui::Key::R, MyApp::reset);
        func.insert(egui::Key::O, MyApp::open);
        func.insert(egui::Key::S, MyApp::save);
        func.insert(egui::Key::C, MyApp::comparison);
        func.insert(egui::Key::H, MyApp::hide);

        let keys: Vec<_> = func.keys().cloned().collect();

        let mut frame = egui::Frame::default();
        frame.outer_margin = egui::style::Margin::same(0.0);
        frame.inner_margin = egui::style::Margin::same(0.0);

        Self {
            stimer,
            layout,
            renderer,
            keys,
            func,
            frame,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.hotkey(ctx);
        if ctx.input().key_pressed(egui::Key::Space) {}

        let image = self.render(frame);
        let cp = egui::CentralPanel::default().frame(self.frame);
        cp.show(ctx, |ui| {
            let texture = ctx.load_texture("render", image, Default::default());
            ui.image(&texture, texture.size_vec2());
        });
        ctx.request_repaint_after(Duration::from_millis(10));
    }
}
