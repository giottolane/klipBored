use relm4::prelude::*;
use adw::prelude::*;
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;
use std::time::{Duration, Instant};
use gtk::{gdk, glib, pango, gio};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::cell::RefCell;
use std::process::Command;
use std::fs;

const APP_CSS: &str = include_str!("style.css");

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(APP_CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

fn config_file() -> std::path::PathBuf {
    glib::user_config_dir().join("klipBored").join("keybinding")
}

fn has_keybinding() -> bool {
    config_file().exists() && fs::read_to_string(config_file()).map_or(false, |s| !s.trim().is_empty())
}

fn save_keybinding(binding: &str) {
    let path = config_file();
    let _ = fs::create_dir_all(path.parent().unwrap());
    let _ = fs::write(path, binding);
}

#[derive(Debug, Clone, PartialEq)]
struct ImageDataOwned {
    width: usize, height: usize, data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
enum ClipboardContent {
    Text { full: String, display: String },
    Image { texture: gdk::Texture, raw: ImageDataOwned },
}

impl ClipboardEntry {
    fn view_mode(&self) -> &str {
        match &self.content {
            ClipboardContent::Text { .. } => "text_page",
            ClipboardContent::Image { .. } => "image_page",
        }
    }
    fn display_text(&self) -> String {
        match &self.content {
            ClipboardContent::Text { display, .. } => display.clone(),
            _ => String::new(),
        }
    }
    fn texture(&self) -> Option<gdk::Paintable> {
        match &self.content {
            ClipboardContent::Image { texture, .. } => Some(texture.clone().upcast()),
            _ => None,
        }
    }
}

fn compact_preview(text: &str) -> String {
    let max_lines = 4;
    let max_chars = 300;
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > max_lines { lines.truncate(max_lines); }
    let mut result = lines.join("\n");
    if result.len() > max_chars { result = result.chars().take(max_chars).collect(); }
    result
}

fn raw_to_texture(width: i32, height: i32, data: &[u8]) -> gdk::Texture {
    let bytes = glib::Bytes::from(data);
    gdk::MemoryTexture::new(width, height, gdk::MemoryFormat::R8g8b8a8, &bytes, width as usize * 4).upcast()
}

#[derive(Debug)]
struct ClipboardEntry { content: ClipboardContent }

#[derive(Debug)]
enum ClipboardEntryOutput { RequestCopy(DynamicIndex), DeleteItem(DynamicIndex) }

#[relm4::factory]
impl FactoryComponent for ClipboardEntry {
    type Init = ClipboardContent;
    type Input = ();
    type Output = ClipboardEntryOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Horizontal,
            set_spacing: 12,
            add_css_class: "clipboard-row",
            set_valign: gtk::Align::Start,

            gtk::Stack {
                set_hexpand: true,
                set_valign: gtk::Align::Center,

                add_named[Some("text_page")] = &gtk::Label {
                    set_halign: gtk::Align::Start,
                    set_wrap: true,
                    set_wrap_mode: pango::WrapMode::WordChar,
                    set_ellipsize: pango::EllipsizeMode::End,
                    set_lines: 4,
                    set_xalign: 0.0,
                    #[watch]
                    set_label: &self.display_text(),
                },

                add_named[Some("image_page")] = &gtk::Picture {
                    set_keep_aspect_ratio: true,
                    set_can_shrink: true,
                    set_height_request: 100,
                    add_css_class: "clipboard-img",
                    #[watch]
                    set_paintable: self.texture().as_ref(),
                },

                #[watch]
                set_visible_child_name: self.view_mode(),
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 8,
                gtk::Button {
                    set_icon_name: "edit-copy-symbolic",
                    add_css_class: "copy-btn",
                    connect_clicked[sender, index] => move |_| {
                        sender.output(ClipboardEntryOutput::RequestCopy(index.clone())).unwrap();
                    }
                },
                gtk::Button {
                    set_icon_name: "user-trash-symbolic",
                    add_css_class: "delete-btn",
                    connect_clicked[sender, index] => move |_| {
                        sender.output(ClipboardEntryOutput::DeleteItem(index.clone())).unwrap();
                    }
                },
            }
        }
    }
    fn init_model(content: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self { Self { content } }
}

struct ClipboardTracker { last_text: String, last_img_hash: u64, last_own_copy: Instant }
struct KlipBoredModel {
    clipboard_entries: FactoryVecDeque<ClipboardEntry>,
    tracker: Rc<RefCell<ClipboardTracker>>,
    setup_done: Rc<RefCell<bool>>,
    current_page: String, // "wizard", "wizard_custom", "clipboard"
}

#[derive(Debug)]
enum KlipBoredMsg {
    NewItem(ClipboardContent),
    RequestCopy(DynamicIndex),
    DeleteItem(DynamicIndex),
    WizardAccept,
    WizardShowCustom,
    WizardApplyBinding(String),
}

#[relm4::component]
impl SimpleComponent for KlipBoredModel {
    type Init = ();
    type Input = KlipBoredMsg;
    type Output = ();

    view! {
        root = adw::ApplicationWindow {
            set_title: Some("klipBored"),
            set_default_size: (360, 500),
            set_decorated: false,
            set_icon_name: Some("klipbored"),


            #[wrap(Some)]
            set_content = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                add_css_class: "main-window",

                adw::HeaderBar {
                    set_show_end_title_buttons: true,
                },

                gtk::Stack {
                    set_vexpand: true,
                    set_transition_type: gtk::StackTransitionType::SlideLeftRight,
                    set_transition_duration: 250,
                    #[watch]
                    set_visible_child_name: &model.current_page,

                    // --- Página 1: Bienvenida ---
                    add_named[Some("wizard")] = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,
                        set_spacing: 24,
                        set_margin_start: 32,
                        set_margin_end: 32,
                        set_margin_top: 24,
                        set_margin_bottom: 32,

                        gtk::Image {
                            set_icon_name: Some("edit-paste-symbolic"),
                            set_pixel_size: 64,
                            add_css_class: "wizard-icon",
                        },

                        gtk::Label {
                            set_label: "Bienvenido a klipBored",
                            add_css_class: "wizard-title",
                        },

                        gtk::Label {
                            set_label: "Tu historial de portapapeles inteligente.\nPara acceder rápidamente, podemos configurar\nel atajo de teclado.",
                            set_justify: gtk::Justification::Center,
                            set_wrap: true,
                            add_css_class: "wizard-description",
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 10,
                            set_halign: gtk::Align::Center,

                            gtk::Button {
                                set_label: "Usar  Win + V",
                                add_css_class: "wizard-btn-primary",
                                set_width_request: 220,
                                connect_clicked => KlipBoredMsg::WizardAccept,
                            },

                            gtk::Button {
                                set_label: "Elegir otro atajo",
                                add_css_class: "wizard-btn-secondary",
                                set_width_request: 220,
                                connect_clicked => KlipBoredMsg::WizardShowCustom,
                            },
                        },
                    },

                    // --- Página 2: Elegir atajo personalizado ---
                    add_named[Some("wizard_custom")] = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,
                        set_spacing: 20,
                        set_margin_start: 32,
                        set_margin_end: 32,
                        set_margin_top: 24,
                        set_margin_bottom: 32,

                        gtk::Image {
                            set_icon_name: Some("preferences-desktop-keyboard-shortcuts-symbolic"),
                            set_pixel_size: 48,
                            add_css_class: "wizard-icon",
                        },

                        gtk::Label {
                            set_label: "Elige tu atajo",
                            add_css_class: "wizard-title",
                        },

                        gtk::Label {
                            set_label: "Selecciona una combinación de teclas\npara abrir klipBored.",
                            set_justify: gtk::Justification::Center,
                            set_wrap: true,
                            add_css_class: "wizard-description",
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 8,
                            set_halign: gtk::Align::Center,

                            gtk::Button {
                                add_css_class: "wizard-shortcut-option",
                                set_width_request: 240,
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_halign: gtk::Align::Center,
                                    gtk::Label { set_label: "Ctrl + Shift + V", add_css_class: "shortcut-key" },
                                },
                                connect_clicked => KlipBoredMsg::WizardApplyBinding("<Control><Shift>v".to_string()),
                            },

                            gtk::Button {
                                add_css_class: "wizard-shortcut-option",
                                set_width_request: 240,
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_halign: gtk::Align::Center,
                                    gtk::Label { set_label: "Super + C", add_css_class: "shortcut-key" },
                                },
                                connect_clicked => KlipBoredMsg::WizardApplyBinding("<Super>c".to_string()),
                            },

                            gtk::Button {
                                add_css_class: "wizard-shortcut-option",
                                set_width_request: 240,
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_halign: gtk::Align::Center,
                                    gtk::Label { set_label: "Ctrl + Super + V", add_css_class: "shortcut-key" },
                                },
                                connect_clicked => KlipBoredMsg::WizardApplyBinding("<Control><Super>v".to_string()),
                            },
                        },

                    },

                    // --- Página del Clipboard ---
                    add_named[Some("clipboard")] = &gtk::ScrolledWindow {
                        set_vexpand: true,
                        #[local_ref]
                        list_box -> gtk::ListBox {
                            add_css_class: "content-list",
                        }
                    },
                },
            }
        }
    }

    fn init(_: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let needs_setup = !has_keybinding();
        let setup_done = Rc::new(RefCell::new(!needs_setup));
        let root_ref = root.clone();

        let tracker = Rc::new(RefCell::new(ClipboardTracker {
            last_text: String::new(), last_img_hash: 0, last_own_copy: Instant::now() - Duration::from_secs(5),
        }));

        let clipboard_entries = FactoryVecDeque::builder()
            .launch_default()
            .forward(sender.input_sender(), |output| match output {
                ClipboardEntryOutput::RequestCopy(index) => KlipBoredMsg::RequestCopy(index),
                ClipboardEntryOutput::DeleteItem(index) => KlipBoredMsg::DeleteItem(index),
            });

        // Polling del clipboard: solo activo si setup_done es true
        let tracker_loop = tracker.clone();
        let setup_done_loop = setup_done.clone();
        let s_clone = sender.clone();
        glib::timeout_add_local(Duration::from_millis(800), move || {
            if !*setup_done_loop.borrow() { return glib::ControlFlow::Continue; }

            let mut state = tracker_loop.borrow_mut();
            if state.last_own_copy.elapsed() < Duration::from_millis(1500) { return glib::ControlFlow::Continue; }
            if let Ok(mut cb) = Clipboard::new() {
                if let Ok(text) = cb.get_text() {
                    if !text.is_empty() && text != state.last_text {
                        state.last_text = text.clone();
                        s_clone.input(KlipBoredMsg::NewItem(ClipboardContent::Text { full: text.clone(), display: compact_preview(&text) }));
                        return glib::ControlFlow::Continue;
                    }
                }
                if let Ok(img) = cb.get_image() {
                    let h = calculate_hash(&img.bytes);
                    if img.bytes.len() > 0 && h != state.last_img_hash {
                        state.last_img_hash = h;
                        let owned = ImageDataOwned { width: img.width, height: img.height, data: img.bytes.into_owned() };
                        let tex = raw_to_texture(owned.width as i32, owned.height as i32, &owned.data);
                        s_clone.input(KlipBoredMsg::NewItem(ClipboardContent::Image { texture: tex, raw: owned }));
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        let current_page = if needs_setup { "wizard".to_string() } else { "clipboard".to_string() };
        let model = KlipBoredModel { clipboard_entries, tracker, setup_done: setup_done.clone(), current_page };
        let list_box = model.clipboard_entries.widget();
        let widgets = view_output!();

        // Escape solo cierra si ya se completó el wizard
        let esc_controller = gtk::EventControllerKey::new();
        let root_for_esc = root_ref.clone();
        let setup_done_esc = setup_done.clone();
        esc_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape && *setup_done_esc.borrow() {
                root_for_esc.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        root_ref.add_controller(esc_controller);

        // Bloquear cierre de ventana durante el wizard
        let setup_done_close = setup_done.clone();
        root_ref.connect_close_request(move |_| {
            if *setup_done_close.borrow() {
                glib::Propagation::Proceed // permitir cerrar
            } else {
                glib::Propagation::Stop // bloquear cierre
            }
        });

        if needs_setup {
            root_ref.set_visible(true);
            root_ref.present();
        } else {
            root_ref.set_visible(false);
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            KlipBoredMsg::WizardAccept => {
                save_keybinding("<Super>v");
                *self.setup_done.borrow_mut() = true;
                self.current_page = "clipboard".to_string();
                if let Ok(p) = std::env::current_exe() {
                    if let Some(s) = p.to_str() {
                        setup_gsettings_binding(s, "<Super>v");
                    }
                }
                let app = gtk::Application::default();
                if let Some(win) = app.active_window() {
                    win.set_visible(false);
                }
            }
            KlipBoredMsg::WizardShowCustom => {
                self.current_page = "wizard_custom".to_string();
            }
            KlipBoredMsg::WizardApplyBinding(binding) => {
                save_keybinding(&binding);
                *self.setup_done.borrow_mut() = true;
                self.current_page = "clipboard".to_string();
                if let Ok(p) = std::env::current_exe() {
                    if let Some(s) = p.to_str() {
                        setup_gsettings_binding(s, &binding);
                    }
                }
                let app = gtk::Application::default();
                if let Some(win) = app.active_window() {
                    win.set_visible(false);
                }
            }
            KlipBoredMsg::NewItem(content) => {
                let mut guard = self.clipboard_entries.guard();
                guard.push_front(content);
                if guard.len() > 50 { guard.pop_back(); }
            }
            KlipBoredMsg::DeleteItem(index) => { self.clipboard_entries.guard().remove(index.current_index()); }
            KlipBoredMsg::RequestCopy(index) => {
                if let Some(entry) = self.clipboard_entries.get(index.current_index()) {
                    let content = entry.content.clone();

                    {
                        let mut state = self.tracker.borrow_mut();
                        state.last_own_copy = Instant::now();
                        match &content {
                            ClipboardContent::Text { full, .. } => state.last_text = full.clone(),
                            ClipboardContent::Image { raw, .. } => state.last_img_hash = calculate_hash(&raw.data),
                        }
                    }

                    let app = gtk::Application::default();
                    if let Some(win) = app.active_window() {
                        win.set_visible(false);
                    }

                    std::thread::spawn(move || {
                        if let Ok(mut cb) = Clipboard::new() {
                            match content {
                                ClipboardContent::Text { full, .. } => { let _ = cb.set_text(full); },
                                ClipboardContent::Image { raw, .. } => {
                                    let data = ImageData { width: raw.width, height: raw.height, bytes: Cow::Borrowed(&raw.data) };
                                    let _ = cb.set_image(data);
                                }
                            }
                            std::thread::sleep(Duration::from_millis(600));
                        }
                    });
                }
            }
        }
    }

}

fn setup_gsettings_binding(path_str: &str, binding: &str) {
    let schema = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
    let path = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/";
    let _ = Command::new("gsettings").args(&["set", &format!("{}:{}", schema, path), "name", "klipBored"]).status();
    let _ = Command::new("gsettings").args(&["set", &format!("{}:{}", schema, path), "command", path_str]).status();
    let _ = Command::new("gsettings").args(&["set", &format!("{}:{}", schema, path), "binding", binding]).status();
    let _ = Command::new("gsettings").args(&["set", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings", &format!("['{}']", path)]).status();
}

fn main() {
    #[cfg(target_os = "linux")]
    glib::unix_signal_add(2, || { std::process::exit(0); });

    let app = adw::Application::builder()
        .application_id("io.github.klipbored.app")
        .flags(gio::ApplicationFlags::FLAGS_NONE)
        .build();

    // Cell<bool> para toggle sin panics por reentrada
    let shown = Rc::new(std::cell::Cell::new(false));

    app.connect_startup(|_| load_css());

    let shown_activate = shown.clone();
    app.connect_activate(move |app| {
        if !has_keybinding() { return; }
        if let Some(window) = app.active_window() {
            if shown_activate.get() {
                shown_activate.set(false);
                window.set_visible(false);
            } else {
                shown_activate.set(true);
                window.set_visible(true);
                window.present();
            }
        }
    });

    // Sincronizar flag cuando se oculta por Escape, copiar, etc.
    let shown_for_app = shown.clone();
    app.connect_window_added(move |_, window| {
        let shown_hide = shown_for_app.clone();
        window.connect_hide(move |_| {
            shown_hide.set(false);
        });
    });

    RelmApp::from_app(app).run::<KlipBoredModel>(());
}
