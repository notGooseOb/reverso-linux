mod app;
mod config;
mod editor;
mod lang;
mod translator;
mod ui;

use app::{App, UserEvent};
use std::io::{IsTerminal, Read};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

fn usage(prog: &str) {
    eprintln!(
        "Usage: {prog} [options] [text...]\n\n\
         Options:\n  \
         -t, --target LANG   Default target language\n  \
         -v, --version       Show version\n  \
         -h, --help          Show this help\n\n\
         Source and target are remembered between sessions.\n\
         Examples:\n  \
         {prog} hello world\n  \
         wl-paste -p | {prog}"
    );
}

fn read_stdin_text() -> Option<String> {
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).ok()?;
    let trimmed = buf.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(editor::MAX_TEXT).collect())
}

fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("VmRSS:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse::<f64>().ok())
        })
        .map(|kb| kb / 1024.0)
        .unwrap_or(0.0)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let prog = std::env::args()
        .next()
        .unwrap_or_else(|| "reverso-linux".into());

    let mut help = false;
    let mut version = false;
    let mut bench = false;
    let mut target_arg: Option<String> = None;
    let mut words: Vec<String> = Vec::new();
    let mut after_dash_dash = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !after_dash_dash {
            match arg.as_str() {
                "--" => {
                    after_dash_dash = true;
                    i += 1;
                    continue;
                }
                "-h" | "--help" => {
                    help = true;
                    i += 1;
                    continue;
                }
                "-v" | "--version" => {
                    version = true;
                    i += 1;
                    continue;
                }
                "--bench" => {
                    bench = true;
                    i += 1;
                    continue;
                }
                "-t" | "--target" => {
                    i += 1;
                    if i >= args.len() {
                        usage(&prog);
                        std::process::exit(1);
                    }
                    target_arg = Some(args[i].clone());
                    i += 1;
                    continue;
                }
                s if s.starts_with("--target=") => {
                    target_arg = Some(s["--target=".len()..].to_string());
                    i += 1;
                    continue;
                }
                s if s.starts_with('-') && s.len() > 1 => {
                    eprintln!("Unknown option: {s}\n");
                    usage(&prog);
                    std::process::exit(1);
                }
                _ => {}
            }
        }
        words.push(arg.clone());
        i += 1;
    }

    if help {
        usage(&prog);
        return;
    }
    if version {
        println!("reverso-linux {}", env!("APP_VERSION"));
        return;
    }

    if let Some(target) = &target_arg {
        config::save_target(target);
    }

    let mut text = if words.is_empty() {
        if std::io::stdin().is_terminal() {
            None
        } else {
            read_stdin_text()
        }
    } else {
        Some(words.join(" "))
    };

    let cfg = config::load();
    let target_name = target_arg
        .or(cfg.target_lang.clone())
        .unwrap_or_else(|| "russian".to_string());
    let target_name = if lang::is_valid(&target_name) {
        target_name
    } else {
        "russian".to_string()
    };
    let source_name = cfg
        .last_source_lang
        .clone()
        .filter(|source| lang::is_valid(source) && !source.eq_ignore_ascii_case(&target_name))
        .unwrap_or_else(|| {
            if target_name.eq_ignore_ascii_case("english") {
                "russian".to_string()
            } else {
                "english".to_string()
            }
        });

    if text.is_none() && !bench {
        // Matches the old CLI: no input means nothing to show.
        return;
    }
    if text.is_none() {
        text = Some("hello".to_string());
    }

    let source_idx = lang::index_or(&source_name, 0);
    let target_idx = lang::index_or(&target_name, 1);

    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(err) => {
            eprintln!("Failed to create event loop: {err}");
            std::process::exit(1);
        }
    };
    event_loop.set_control_flow(ControlFlow::Wait);

    let proxy = event_loop.create_proxy();
    let mut app = App::new(
        proxy,
        text.unwrap_or_default(),
        source_idx,
        target_idx,
        bench,
    );

    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("Event loop failed: {err}");
        std::process::exit(1);
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        self.create_window(event_loop);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Translated {
                result,
                source,
                target,
            } => self.on_translated(result, source, target),
            UserEvent::Examples {
                result,
                option,
                request,
            } => self.on_examples(result, option, request),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => self.request_redraw(),
            WindowEvent::ScaleFactorChanged { .. } => self.request_redraw(),
            WindowEvent::RedrawRequested => {
                self.draw();
                if self.bench && !self.first_done {
                    self.first_done = true;
                    let elapsed = self.start.elapsed().as_secs_f64() * 1000.0;
                    println!("first frame: {:.1} ms, RSS {:.1} MB", elapsed, rss_mb());
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.on_cursor_moved(position.x, position.y);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.on_mouse_input(event_loop, state, button);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self
                    .window
                    .as_ref()
                    .map(|window| window.scale_factor())
                    .unwrap_or(1.0);
                self.on_wheel(delta, scale);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.mods = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event_loop, event),
            WindowEvent::Ime(ime) => match ime {
                Ime::Preedit(text, _) => {
                    self.preedit = text;
                    self.request_redraw();
                }
                Ime::Commit(text) => {
                    self.preedit.clear();
                    self.editor.insert(&text);
                    self.caret_on = true;
                    self.next_blink = std::time::Instant::now() + Duration::from_millis(530);
                    self.request_redraw();
                }
                Ime::Enabled => {}
                Ime::Disabled => {
                    self.preedit.clear();
                    self.request_redraw();
                }
            },
            WindowEvent::Focused(focused) => {
                self.focused = focused;
                self.caret_on = true;
                self.next_blink = std::time::Instant::now() + Duration::from_millis(530);
                self.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.focused || self.window.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = std::time::Instant::now();
        if now >= self.next_blink {
            self.caret_on = !self.caret_on;
            self.next_blink = now + Duration::from_millis(530);
            self.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_blink));
    }
}
