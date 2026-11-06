use crate::config;
use crate::editor::Editor;
use crate::lang;
use crate::translator;
use crate::ui::Layout;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowLevel};

pub enum UserEvent {
    Translated {
        result: Result<translator::Response, String>,
        source: String,
        target: String,
    },
    Examples {
        result: Result<Vec<translator::Example>, String>,
        option: usize,
        request: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Source,
    Target,
}

pub struct Dropdown {
    pub side: Side,
    pub query: String,
    /// Index into the filtered language list, not into `lang::LANGS`.
    pub highlight: Option<usize>,
}

pub struct App {
    pub window: Option<Arc<Window>>,
    pub surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    pub context: Option<softbuffer::Context<Arc<Window>>>,

    pub proxy: EventLoopProxy<UserEvent>,
    pub editor: Editor,
    pub source_idx: usize,
    pub target_idx: usize,

    pub response: Option<translator::Response>,
    pub busy: bool,
    pub error: Option<String>,
    pub selected_option: Option<usize>,
    pub dropdown: Option<Dropdown>,

    pub layout: Layout,
    pub scroll: f64,
    pub mouse: (f64, f64),
    pub dragging: bool,
    pub pending_cursor: Option<f64>,
    pub pending_anchor: bool,

    pub preedit: String,
    pub ime_area: Option<(f64, f64, f64)>,
    pub caret_on: bool,
    pub next_blink: Instant,
    pub focused: bool,
    pub mods: ModifiersState,

    pub clipboard: Option<arboard::Clipboard>,
    pub pixels: Vec<u8>,

    pub examples_request: u64,
    pub examples_loading: Option<usize>,

    pub bench: bool,
    pub first_done: bool,
    pub start: Instant,
    pub started_translation: bool,
}

impl App {
    pub fn new(
        proxy: EventLoopProxy<UserEvent>,
        text: String,
        source_idx: usize,
        target_idx: usize,
        bench: bool,
    ) -> Self {
        let mut editor = Editor::default();
        editor.set_text(&text);

        let response = bench.then(|| translator::Response {
            input: text.clone(),
            translations: vec![
                "привет".to_string(),
                "здравствуйте".to_string(),
                "алло".to_string(),
            ],
            options: vec![
                translator::TranslationOption {
                    translation: "привет".to_string(),
                    frequency: 95,
                    count: 3,
                    examples: vec![
                        translator::Example {
                            source: "Hello, how are you?".to_string(),
                            target: "Привет, как дела?".to_string(),
                        },
                        translator::Example {
                            source: "Hello there, my friend.".to_string(),
                            target: "Привет, мой друг.".to_string(),
                        },
                        translator::Example {
                            source: "Say hello to your mother for me.".to_string(),
                            target: "Передай привет своей маме.".to_string(),
                        },
                    ],
                },
                translator::TranslationOption {
                    translation: "здравствуйте".to_string(),
                    frequency: 32,
                    count: 2,
                    examples: vec![translator::Example {
                        source: "Hello, sir.".to_string(),
                        target: "Здравствуйте, сэр.".to_string(),
                    }],
                },
                translator::TranslationOption {
                    translation: "алло".to_string(),
                    frequency: 9,
                    count: 1,
                    examples: vec![translator::Example {
                        source: "Hello? Can you hear me?".to_string(),
                        target: "Алло? Вы меня слышите?".to_string(),
                    }],
                },
            ],
        });

        Self {
            window: None,
            surface: None,
            context: None,
            proxy,
            editor,
            source_idx,
            target_idx,
            response,
            busy: false,
            error: None,
            selected_option: None,
            dropdown: None,
            layout: Layout::default(),
            scroll: 0.0,
            mouse: (0.0, 0.0),
            dragging: false,
            pending_cursor: None,
            pending_anchor: false,
            preedit: String::new(),
            ime_area: None,
            caret_on: true,
            next_blink: Instant::now() + Duration::from_millis(530),
            focused: true,
            mods: ModifiersState::empty(),
            clipboard: None,
            pixels: Vec::new(),
            examples_request: 0,
            examples_loading: None,
            bench,
            first_done: false,
            start: Instant::now(),
            started_translation: false,
        }
    }

    pub fn request_redraw(&self) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    pub fn start_translate(&mut self) {
        if self.busy {
            return;
        }
        let text = self.editor.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        let source = lang::name(self.source_idx).to_string();
        let target = lang::name(self.target_idx).to_string();

        self.busy = true;
        self.error = None;
        self.response = None;
        self.selected_option = None;
        self.examples_loading = None;
        self.examples_request += 1;
        self.scroll = 0.0;
        config::save(Some(&target), Some(&source));

        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = translator::translate(&text, &source, &target);
            let _ = proxy.send_event(UserEvent::Translated {
                result,
                source,
                target,
            });
        });
    }

    pub fn on_translated(
        &mut self,
        result: Result<translator::Response, String>,
        source: String,
        target: String,
    ) {
        self.busy = false;
        match result {
            Ok(response) => {
                self.selected_option = if response.options.len() == 1 {
                    Some(0)
                } else {
                    None
                };
                self.response = Some(response);
                self.error = None;
            }
            Err(err) => {
                self.response = None;
                self.error = Some(err);
            }
        }
        config::save(Some(&target), Some(&source));

        let autofetch = self.response.as_ref().and_then(|response| {
            let chips = translator::is_short_text(&response.input) && response.options.len() > 1;
            if chips {
                None
            } else if response
                .options
                .first()
                .is_some_and(|option| option.examples.is_empty())
            {
                Some(0)
            } else {
                None
            }
        });
        if let Some(option) = autofetch {
            self.fetch_examples_for(option);
        }
        self.request_redraw();
    }

    pub fn fetch_examples_for(&mut self, option: usize) {
        if self.busy {
            return;
        }
        let Some(response) = &self.response else {
            return;
        };
        let Some(opt) = response.options.get(option) else {
            return;
        };
        if !opt.examples.is_empty() {
            return;
        }
        let text = response.input.clone();
        let fragment = opt.translation.clone();
        if fragment.is_empty() {
            return;
        }
        let source = lang::name(self.source_idx).to_string();
        let target = lang::name(self.target_idx).to_string();

        self.examples_request += 1;
        let request = self.examples_request;
        self.examples_loading = Some(option);

        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = translator::fetch_examples(&text, &source, &target, &fragment);
            let _ = proxy.send_event(UserEvent::Examples {
                result,
                option,
                request,
            });
        });
        self.request_redraw();
    }

    pub fn on_examples(
        &mut self,
        result: Result<Vec<translator::Example>, String>,
        option: usize,
        request: u64,
    ) {
        if request != self.examples_request {
            return;
        }
        self.examples_loading = None;
        if let Ok(examples) = result {
            if !examples.is_empty() {
                if let Some(response) = self.response.as_mut() {
                    if let Some(opt) = response.options.get_mut(option) {
                        opt.examples = examples;
                    }
                }
            }
        }
        self.request_redraw();
    }

    fn selected_translation(&self) -> Option<String> {
        let response = self.response.as_ref()?;
        if let Some(opt) = self
            .selected_option
            .and_then(|i| response.options.get(i))
            .or_else(|| response.options.first())
        {
            return Some(opt.translation.clone());
        }
        response.translations.first().cloned()
    }

    pub fn swap(&mut self) {
        if self.busy {
            return;
        }
        let text = self.selected_translation();
        std::mem::swap(&mut self.source_idx, &mut self.target_idx);
        if self.source_idx == self.target_idx {
            let fallback = if lang::name(self.target_idx) == "english" {
                "russian"
            } else {
                "english"
            };
            self.source_idx = lang::index_or(fallback, 0);
        }
        if let Some(text) = text {
            self.editor.set_text(&text);
        }
        self.selected_option = None;
        self.start_translate();
    }

    pub fn set_lang(&mut self, side: Side, idx: usize) {
        let old_source = self.source_idx;
        let old_target = self.target_idx;
        match side {
            Side::Source => {
                self.source_idx = idx;
                if self.source_idx == self.target_idx {
                    self.target_idx = old_source;
                }
            }
            Side::Target => {
                self.target_idx = idx;
                if self.target_idx == self.source_idx {
                    self.source_idx = old_target;
                }
            }
        }
        self.close_dropdown();
        self.selected_option = None;
        if !self.editor.text.trim().is_empty() {
            self.start_translate();
        }
    }

    fn open_dropdown(&mut self, side: Side) {
        let idx = match side {
            Side::Source => self.source_idx,
            Side::Target => self.target_idx,
        };
        self.preedit.clear();
        self.dropdown = Some(Dropdown {
            side,
            query: String::new(),
            highlight: Some(idx),
        });
        self.caret_on = true;
        self.next_blink = Instant::now() + Duration::from_millis(530);
        if let Some(window) = &self.window {
            window.set_ime_allowed(false);
        }
    }

    fn close_dropdown(&mut self) {
        if self.dropdown.take().is_some() {
            self.preedit.clear();
            if let Some(window) = &self.window {
                window.set_ime_allowed(true);
            }
        }
    }

    fn dropdown_push_text(&mut self, text: &str) {
        if let Some(dropdown) = self.dropdown.as_mut() {
            for ch in text.chars() {
                if ch.is_control() {
                    continue;
                }
                if dropdown.query.chars().count() >= 40 {
                    break;
                }
                dropdown.query.push(ch);
            }
            dropdown.highlight = if lang::filter(&dropdown.query).is_empty() {
                None
            } else {
                Some(0)
            };
        }
        self.caret_on = true;
        self.next_blink = Instant::now() + Duration::from_millis(530);
    }

    fn dropdown_backspace(&mut self) {
        if let Some(dropdown) = self.dropdown.as_mut() {
            if dropdown.query.pop().is_some() {
                dropdown.highlight = if lang::filter(&dropdown.query).is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
        self.caret_on = true;
        self.next_blink = Instant::now() + Duration::from_millis(530);
    }

    fn dropdown_move(&mut self, down: bool) {
        let Some(dropdown) = self.dropdown.as_mut() else {
            return;
        };
        let count = lang::filter(&dropdown.query).len();
        if count == 0 {
            dropdown.highlight = None;
            return;
        }
        let current = dropdown.highlight.unwrap_or(0).min(count - 1);
        dropdown.highlight = Some(if down {
            (current + 1) % count
        } else {
            (current + count - 1) % count
        });
    }

    fn dropdown_accept(&mut self) {
        let selection = self.dropdown.as_ref().and_then(|dropdown| {
            let filtered = lang::filter(&dropdown.query);
            let highlight = dropdown.highlight?;
            filtered
                .get(highlight)
                .copied()
                .map(|language| (dropdown.side, language))
        });
        if let Some((side, language)) = selection {
            self.set_lang(side, language);
        }
    }

    fn clipboard_get(&mut self) -> Option<String> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(cb) = self.clipboard.as_mut() {
            if let Ok(text) = cb.get_text() {
                return Some(text);
            }
        }
        std::process::Command::new("wl-paste")
            .arg("--no-newline")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
    }

    fn clipboard_set(&mut self, text: &str) {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        if let Some(cb) = self.clipboard.as_mut() {
            let _ = cb.set_text(text.to_string());
        } else {
            let _ = std::process::Command::new("wl-copy").arg(text).spawn();
        }
    }

    pub fn on_key(&mut self, event_loop: &ActiveEventLoop, event: KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        let ctrl = self.mods.control_key();
        let alt = self.mods.alt_key();
        let shift = self.mods.shift_key();

        if self.dropdown.is_some() {
            match &event.logical_key {
                Key::Named(NamedKey::Escape) => {
                    self.close_dropdown();
                }
                Key::Named(NamedKey::ArrowDown) => self.dropdown_move(true),
                Key::Named(NamedKey::ArrowUp) => self.dropdown_move(false),
                Key::Named(NamedKey::Enter) => self.dropdown_accept(),
                Key::Named(NamedKey::Backspace) => self.dropdown_backspace(),
                _ => {
                    if ctrl && !alt {
                        if let Key::Character(ch) = &event.logical_key {
                            if ch.eq_ignore_ascii_case("v") {
                                if let Some(text) = self.clipboard_get() {
                                    self.dropdown_push_text(&text);
                                }
                            }
                        }
                    } else if let Some(text) = &event.text {
                        if !text.is_empty() && !text.chars().any(char::is_control) {
                            self.dropdown_push_text(text);
                        }
                    }
                }
            }
            self.request_redraw();
            return;
        }

        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                event_loop.exit();
                return;
            }
            Key::Named(NamedKey::Backspace) => self.editor.backspace(),
            Key::Named(NamedKey::Delete) => self.editor.delete(),
            Key::Named(NamedKey::ArrowLeft) => self.editor.left(shift),
            Key::Named(NamedKey::ArrowRight) => self.editor.right(shift),
            Key::Named(NamedKey::Home) => self.editor.home(shift),
            Key::Named(NamedKey::End) => self.editor.end(shift),
            Key::Named(NamedKey::Enter) => self.start_translate(),
            Key::Named(NamedKey::Tab) => {}
            _ => {
                if ctrl && !alt {
                    if let Key::Character(ch) = &event.logical_key {
                        match ch.to_lowercase().as_str() {
                            "a" => self.editor.select_all(),
                            "c" => {
                                let text = self.editor.copy();
                                self.clipboard_set(&text);
                            }
                            "x" => {
                                let text = self.editor.cut();
                                self.clipboard_set(&text);
                            }
                            "v" => {
                                if let Some(text) = self.clipboard_get() {
                                    self.editor.insert(&text);
                                }
                            }
                            _ => {}
                        }
                    }
                } else if let Some(text) = &event.text {
                    if !text.is_empty() && !text.chars().any(char::is_control) {
                        self.editor.insert(text);
                        self.caret_on = true;
                        self.next_blink = Instant::now() + Duration::from_millis(530);
                    }
                }
            }
        }
        self.request_redraw();
    }

    pub fn on_ime_preedit(&mut self, text: String) {
        if self.dropdown.is_some() {
            return;
        }
        self.preedit = text;
        self.request_redraw();
    }

    pub fn on_ime_commit(&mut self, text: String) {
        if self.dropdown.is_some() {
            self.dropdown_push_text(&text);
            self.request_redraw();
            return;
        }
        self.preedit.clear();
        self.editor.insert(&text);
        self.caret_on = true;
        self.next_blink = Instant::now() + Duration::from_millis(530);
        self.request_redraw();
    }

    pub fn on_press(&mut self, event_loop: &ActiveEventLoop, x: f64, y: f64) {
        if self.dropdown.is_some() {
            let hit = self
                .layout
                .dropdown_items
                .iter()
                .position(|r| r.contains(x, y))
                .and_then(|vis| self.layout.dropdown_langs.get(vis).copied());
            if let Some(lang_idx) = hit {
                let side = self
                    .dropdown
                    .as_ref()
                    .map(|d| d.side)
                    .unwrap_or(Side::Source);
                self.set_lang(side, lang_idx);
                self.request_redraw();
                return;
            }
            if self.layout.from.contains(x, y) {
                self.open_dropdown(Side::Source);
                self.request_redraw();
                return;
            }
            if self.layout.to.contains(x, y) {
                self.open_dropdown(Side::Target);
                self.request_redraw();
                return;
            }
            if !self.layout.dropdown.contains(x, y) {
                self.close_dropdown();
                self.request_redraw();
            }
            return;
        }

        if self.layout.from.contains(x, y) {
            self.open_dropdown(Side::Source);
            self.request_redraw();
            return;
        }
        if self.layout.to.contains(x, y) {
            self.open_dropdown(Side::Target);
            self.request_redraw();
            return;
        }
        if self.layout.swap.contains(x, y) {
            self.swap();
            self.request_redraw();
            return;
        }
        if self.layout.translate.contains(x, y) {
            self.start_translate();
            self.request_redraw();
            return;
        }
        if self.layout.close.contains(x, y) {
            event_loop.exit();
            return;
        }
        if self.layout.entry.contains(x, y) {
            self.dragging = true;
            self.pending_cursor = Some(x);
            self.pending_anchor = true;
            self.request_redraw();
            return;
        }
        if let Some(idx) = self.layout.chips.iter().position(|r| r.contains(x, y)) {
            self.selected_option = Some(idx);
            self.fetch_examples_for(idx);
            self.request_redraw();
        }
    }

    pub fn on_cursor_moved(&mut self, x: f64, y: f64) {
        self.mouse = (x, y);
        if self.dragging {
            self.pending_cursor = Some(x);
        }
        if self.dropdown.is_some() {
            if let Some(vis) = self
                .layout
                .dropdown_items
                .iter()
                .position(|r| r.contains(x, y))
            {
                if let Some(dropdown) = self.dropdown.as_mut() {
                    dropdown.highlight = Some(vis);
                }
            }
        }
        self.request_redraw();
    }

    pub fn on_mouse_input(
        &mut self,
        event_loop: &ActiveEventLoop,
        state: ElementState,
        button: MouseButton,
    ) {
        if button != MouseButton::Left {
            return;
        }
        match state {
            ElementState::Pressed => self.on_press(event_loop, self.mouse.0, self.mouse.1),
            ElementState::Released => {
                self.dragging = false;
                self.pending_anchor = false;
            }
        }
    }

    pub fn on_wheel(&mut self, delta: MouseScrollDelta, scale: f64) {
        if self.dropdown.is_some() {
            return;
        }
        let dy = match delta {
            MouseScrollDelta::LineDelta(_, y) => y as f64 * 30.0 * scale,
            MouseScrollDelta::PixelDelta(pos) => pos.y,
        };
        self.scroll -= dy;
        let max = (self.layout.content_height - self.layout.content.h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
        self.request_redraw();
    }

    pub fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        use winit::platform::x11::WindowAttributesExtX11 as _;

        let attributes = Window::default_attributes()
            .with_title("Reverso Linux")
            .with_inner_size(winit::dpi::LogicalSize::new(640.0, 640.0))
            .with_name("reverso-linux", "reverso-linux");

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("Failed to create window: {err}");
                event_loop.exit();
                return;
            }
        };

        window.set_window_level(WindowLevel::AlwaysOnTop);
        window.set_ime_allowed(true);

        let context = match softbuffer::Context::new(window.clone()) {
            Ok(context) => context,
            Err(err) => {
                eprintln!("Failed to create softbuffer context: {err}");
                event_loop.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(surface) => surface,
            Err(err) => {
                eprintln!("Failed to create softbuffer surface: {err}");
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
        self.request_redraw();

        if !self.bench && !self.started_translation && !self.editor.text.trim().is_empty() {
            self.started_translation = true;
            self.start_translate();
        }
    }
}
