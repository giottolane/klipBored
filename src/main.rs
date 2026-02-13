use adw::prelude::*;
use arboard::{Clipboard, ImageData};
use gtk::{gdk, gio, glib, pango};
use relm4::prelude::*;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

const APP_CSS: &str = include_str!("style.css");
const APP_ICON_SVG: &[u8] = include_bytes!("../assets/klipbored.svg");

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

fn app_icon_paintable() -> gdk::Texture {
    let bytes = glib::Bytes::from_static(APP_ICON_SVG);
    gdk::Texture::from_bytes(&bytes).expect("Failed to load embedded SVG icon")
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
    config_file().exists()
        && fs::read_to_string(config_file()).map_or(false, |s| !s.trim().is_empty())
}

fn save_keybinding(binding: &str) {
    let path = config_file();
    let _ = fs::create_dir_all(path.parent().unwrap());
    let _ = fs::write(path, binding);
}

fn get_keybinding() -> String {
    fs::read_to_string(config_file()).unwrap_or_else(|_| "<Super>v".to_string())
}

fn autostart_file() -> std::path::PathBuf {
    glib::user_config_dir()
        .join("autostart")
        .join("io.github.klipbored.app.desktop")
}

fn is_autostart_enabled() -> bool {
    autostart_file().exists()
}

fn set_autostart(enabled: bool) {
    let path = autostart_file();
    if enabled {
        let user_app = glib::user_data_dir()
            .join("applications")
            .join("io.github.klipbored.app.desktop");
        let system_app =
            std::path::Path::new("/usr/share/applications/io.github.klipbored.app.desktop");

        let src = if user_app.exists() {
            Some(user_app)
        } else if system_app.exists() {
            Some(system_app.to_path_buf())
        } else {
            None
        };

        if let Some(src_path) = src {
            let _ = fs::create_dir_all(path.parent().unwrap());
            let _ = fs::copy(src_path, path);
        }
    } else {
        let _ = fs::remove_file(path);
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ImageDataOwned {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
enum ClipboardContent {
    Text {
        full: String,
        display: String,
    },
    Image {
        texture: gdk::Texture,
        raw: ImageDataOwned,
    },
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
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }
    let mut result = lines.join("\n");
    if result.len() > max_chars {
        result = result.chars().take(max_chars).collect();
    }
    result
}

fn raw_to_texture(width: i32, height: i32, data: &[u8]) -> gdk::Texture {
    let bytes = glib::Bytes::from(data);
    gdk::MemoryTexture::new(
        width,
        height,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        width as usize * 4,
    )
    .upcast()
}

#[derive(Debug)]
struct ClipboardEntry {
    content: ClipboardContent,
}

#[derive(Debug)]
enum ClipboardEntryOutput {
    RequestCopy(DynamicIndex),
    DeleteItem(DynamicIndex),
}

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
    fn init_model(content: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        Self { content }
    }
}

struct ClipboardTracker {
    last_text: String,
    last_img_hash: u64,
    last_own_copy: Instant,
}
struct KlipBoredModel {
    clipboard_entries: FactoryVecDeque<ClipboardEntry>,
    tracker: Rc<RefCell<ClipboardTracker>>,
    setup_done: Rc<RefCell<bool>>,
    current_page: String, // "wizard", "wizard_custom", "clipboard", "settings"
    autostart_enabled: bool,
    current_binding: String,
    manual_binding: String,
    binding_status: String, // "ok", "error", "checking"
}

#[derive(Debug)]
enum KlipBoredMsg {
    NewItem(ClipboardContent),
    RequestCopy(DynamicIndex),
    DeleteItem(DynamicIndex),
    WizardAccept,
    WizardShowCustom,
    WizardApplyBinding(String),
    OpenSettings,
    ToggleAutostart(bool),
    BackToClipboard,
    UpdateManualBinding(String),
    ApplyManualBinding,
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
            set_icon_name: Some("io.github.klipbored.app"),


            #[wrap(Some)]
            set_content = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                add_css_class: "main-window",

                adw::HeaderBar {
                    set_show_end_title_buttons: true,

                    #[wrap(Some)]
                    set_title_widget = &gtk::Label {
                        set_label: "klipBored",
                        add_css_class: "header-title",
                    },

                    pack_start = &gtk::Button {
                        set_icon_name: "go-previous-symbolic",
                        #[watch]
                        set_visible: model.current_page == "settings" || model.current_page == "wizard_custom",
                        connect_clicked[sender] => move |_| {
                            sender.input(KlipBoredMsg::BackToClipboard);
                        }
                    },


                    pack_end = &gtk::Button {
                        set_icon_name: "emblem-system-symbolic",
                        #[watch]
                        set_visible: model.current_page == "clipboard",
                        connect_clicked[sender] => move |_| {
                            sender.input(KlipBoredMsg::OpenSettings);
                        }
                    },

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

                        gtk::Picture {
                            set_paintable: Some(&app_icon_paintable()),
                            set_can_shrink: true,
                            set_keep_aspect_ratio: true,
                            set_width_request: 64,
                            set_height_request: 64,
                        },

                        gtk::Label {
                            set_label: "Bienvenido a klipBored",
                            add_css_class: "wizard-title",
                        },

                        gtk::Label {
                            set_label: "Para acceder rápidamente, puedes configurar\nel atajo de teclado.",
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
                                connect_clicked[sender] => move |_| {
                                    sender.input(KlipBoredMsg::WizardAccept);
                                },
                            },

                            gtk::Button {
                                set_label: "Elegir otro atajo",
                                add_css_class: "wizard-btn-secondary",
                                set_width_request: 220,
                                connect_clicked[sender] => move |_| {
                                    sender.input(KlipBoredMsg::WizardShowCustom);
                                },
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

                        gtk::Picture {
                            set_paintable: Some(&app_icon_paintable()),
                            set_can_shrink: true,
                            set_keep_aspect_ratio: true,
                            set_width_request: 48,
                            set_height_request: 48,
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
                                    gtk::Label { set_label: "Win + V", add_css_class: "shortcut-key" },
                                },
                                connect_clicked[sender] => move |_| {
                                    sender.input(KlipBoredMsg::WizardApplyBinding("<Super>v".to_string()));
                                },
                            },

                            gtk::Button {
                                add_css_class: "wizard-shortcut-option",
                                set_width_request: 240,
                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_halign: gtk::Align::Center,
                                    gtk::Label { set_label: "Ctrl + Shift + V", add_css_class: "shortcut-key" },
                                },
                                connect_clicked[sender] => move |_| {
                                    sender.input(KlipBoredMsg::WizardApplyBinding("<Control><Shift>v".to_string()));
                                },
                            },

                            gtk::Separator {
                                set_margin_top: 8,
                                set_margin_bottom: 8,
                            },

                            gtk::Label {
                                set_label: "O introduce uno manualmente:",
                                add_css_class: "wizard-description",
                            },

                            gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 8,
                                add_css_class: "manual-entry-box",

                                gtk::Entry {
                                    set_placeholder_text: Some("<Super>x, <Control>v..."),
                                    set_hexpand: true,
                                    #[watch]
                                    set_text: &model.manual_binding,
                                    connect_changed[sender] => move |e| {
                                        sender.input(KlipBoredMsg::UpdateManualBinding(e.text().to_string()));
                                    },
                                },

                                gtk::Button {
                                    set_label: "Guardar",
                                    add_css_class: "wizard-btn-primary",
                                    #[watch]
                                    set_sensitive: !model.manual_binding.is_empty() && model.binding_status != "error",
                                    connect_clicked[sender] => move |_| {
                                        sender.input(KlipBoredMsg::ApplyManualBinding);
                                    }
                                }
                            },

                            gtk::Label {
                                #[watch]
                                set_label: if model.binding_status == "error" { "Atajo inválido o incompleto" } else { "" },
                                add_css_class: "error-label",
                                #[watch]
                                set_visible: model.binding_status == "error",
                            }
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

                    // --- Página de Ajustes ---
                    add_named[Some("settings")] = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 20,
                        set_margin_all: 24,

                        gtk::Label {
                            set_label: "Ajustes",
                            set_halign: gtk::Align::Start,
                            add_css_class: "settings-section-title",
                        },

                        gtk::ListBox {
                            add_css_class: "boxed-list",
                            set_selection_mode: gtk::SelectionMode::None,

                            adw::ActionRow {
                                set_title: "Arrancar al inicio",
                                set_subtitle: "Abrir klipBored al iniciar sesión",
                                add_suffix = &gtk::Switch {
                                    set_valign: gtk::Align::Center,
                                    #[watch]
                                    set_active: model.autostart_enabled,
                                    connect_state_set[sender] => move |_, state| {
                                        sender.input(KlipBoredMsg::ToggleAutostart(state));
                                        glib::Propagation::Proceed
                                    }
                                }
                            },

                            adw::ActionRow {
                                set_title: "Atajo de teclado",
                                #[watch]
                                set_subtitle: &format!("Actual: {}", model.current_binding.replace("<Super>", "Win + ").replace("<Control>", "Ctrl + ").replace("<Shift>", "Shift + ")),

                                add_suffix = &gtk::Button {
                                    set_label: "Personalizar",
                                    add_css_class: "wizard-btn-secondary",
                                    set_valign: gtk::Align::Center,
                                    connect_clicked[sender] => move |_| {
                                        sender.input(KlipBoredMsg::WizardShowCustom);
                                    }
                                }
                            }
                        },

                        gtk::Box {
                            set_vexpand: true,
                        },

                        gtk::Label {
                            set_label: "klipBored v0.1.0",
                            add_css_class: "version-label",
                        }
                    },
                },

            }
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let needs_setup = !has_keybinding();
        let setup_done = Rc::new(RefCell::new(!needs_setup));
        let root_ref = root.clone();

        let tracker = Rc::new(RefCell::new(ClipboardTracker {
            last_text: String::new(),
            last_img_hash: 0,
            last_own_copy: Instant::now() - Duration::from_secs(5),
        }));

        let clipboard_entries =
            FactoryVecDeque::builder()
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
            if !*setup_done_loop.borrow() {
                return glib::ControlFlow::Continue;
            }

            let mut state = tracker_loop.borrow_mut();
            if state.last_own_copy.elapsed() < Duration::from_millis(1500) {
                return glib::ControlFlow::Continue;
            }
            if let Ok(mut cb) = Clipboard::new() {
                if let Ok(text) = cb.get_text() {
                    if !text.is_empty() && text != state.last_text {
                        state.last_text = text.clone();
                        s_clone.input(KlipBoredMsg::NewItem(ClipboardContent::Text {
                            full: text.clone(),
                            display: compact_preview(&text),
                        }));
                        return glib::ControlFlow::Continue;
                    }
                }
                if let Ok(img) = cb.get_image() {
                    let h = calculate_hash(&img.bytes);
                    if img.bytes.len() > 0 && h != state.last_img_hash {
                        state.last_img_hash = h;
                        let owned = ImageDataOwned {
                            width: img.width,
                            height: img.height,
                            data: img.bytes.into_owned(),
                        };
                        let tex =
                            raw_to_texture(owned.width as i32, owned.height as i32, &owned.data);
                        s_clone.input(KlipBoredMsg::NewItem(ClipboardContent::Image {
                            texture: tex,
                            raw: owned,
                        }));
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        let current_page = if needs_setup {
            "wizard".to_string()
        } else {
            "clipboard".to_string()
        };
        let model = KlipBoredModel {
            clipboard_entries,
            tracker,
            setup_done: setup_done.clone(),
            current_page,
            autostart_enabled: is_autostart_enabled(),
            current_binding: get_keybinding(),
            manual_binding: String::new(),
            binding_status: "ok".to_string(),
        };

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

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
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
                self.current_binding = binding.clone();
                *self.setup_done.borrow_mut() = true;

                if let Ok(p) = std::env::current_exe() {
                    if let Some(s) = p.to_str() {
                        setup_gsettings_binding(s, &binding);
                    }
                }

                if self.current_page == "wizard_custom" {
                    self.current_page = "clipboard".to_string();
                    let app = gtk::Application::default();
                    if let Some(win) = app.active_window() {
                        win.set_visible(false);
                    }
                } else {
                    // If we were in settings, stay in settings or go back
                    self.current_page = "settings".to_string();
                }
            }
            KlipBoredMsg::OpenSettings => {
                self.current_page = "settings".to_string();
                self.autostart_enabled = is_autostart_enabled();
            }
            KlipBoredMsg::ToggleAutostart(enabled) => {
                set_autostart(enabled);
                self.autostart_enabled = enabled;
            }
            KlipBoredMsg::BackToClipboard => {
                if self.current_page == "wizard_custom" && !*self.setup_done.borrow() {
                    self.current_page = "wizard".to_string();
                } else {
                    self.current_page = "clipboard".to_string();
                }
            }

            KlipBoredMsg::UpdateManualBinding(text) => {
                self.manual_binding = text.clone();
                // Validación básica de formato de atajo de GTK
                if text.is_empty() {
                    self.binding_status = "ok".to_string();
                } else if text.contains('<') && text.contains('>') && text.len() > 3 {
                    self.binding_status = "ok".to_string();
                } else {
                    self.binding_status = "error".to_string();
                }
            }

            KlipBoredMsg::ApplyManualBinding => {
                let binding = self.manual_binding.clone();
                sender.input(KlipBoredMsg::WizardApplyBinding(binding));
            }

            KlipBoredMsg::NewItem(content) => {
                let mut guard = self.clipboard_entries.guard();
                guard.push_front(content);
                if guard.len() > 50 {
                    guard.pop_back();
                }
            }
            KlipBoredMsg::DeleteItem(index) => {
                self.clipboard_entries.guard().remove(index.current_index());
            }
            KlipBoredMsg::RequestCopy(index) => {
                if let Some(entry) = self.clipboard_entries.get(index.current_index()) {
                    let content = entry.content.clone();

                    {
                        let mut state = self.tracker.borrow_mut();
                        state.last_own_copy = Instant::now();
                        match &content {
                            ClipboardContent::Text { full, .. } => state.last_text = full.clone(),
                            ClipboardContent::Image { raw, .. } => {
                                state.last_img_hash = calculate_hash(&raw.data)
                            }
                        }
                    }

                    let app = gtk::Application::default();
                    if let Some(win) = app.active_window() {
                        win.set_visible(false);
                    }

                    std::thread::spawn(move || {
                        if let Ok(mut cb) = Clipboard::new() {
                            match content {
                                ClipboardContent::Text { full, .. } => {
                                    let _ = cb.set_text(full);
                                }
                                ClipboardContent::Image { raw, .. } => {
                                    let data = ImageData {
                                        width: raw.width,
                                        height: raw.height,
                                        bytes: Cow::Borrowed(&raw.data),
                                    };
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
    let schema_custom = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
    let schema_main = "org.gnome.settings-daemon.plugins.media-keys";

    // Get current list
    let output = Command::new("gsettings")
        .args(&["get", schema_main, "custom-keybindings"])
        .output();

    let current_list_raw = if let Ok(o) = output {
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    } else {
        "[]".to_string()
    };

    // Simple parsing of "['...', '...']"
    let mut entries: Vec<String> = current_list_raw
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('\'').to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let name_to_find = "klipBored";
    let mut target_path = String::new();

    // Check if it already exists
    for path in &entries {
        let name_output = Command::new("gsettings")
            .args(&["get", &format!("{}:{}", schema_custom, path), "name"])
            .output();
        if let Ok(o) = name_output {
            let name = String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches('\'')
                .to_string();
            if name == name_to_find {
                target_path = path.clone();
                break;
            }
        }
    }

    if target_path.is_empty() {
        // Find next available index
        let mut idx = 0;
        loop {
            let new_path = format!(
                "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom{}/",
                idx
            );
            if !entries.contains(&new_path) {
                target_path = new_path;
                break;
            }
            idx += 1;
        }
        entries.push(target_path.clone());

        // Update the main list
        let formatted_list = format!(
            "[{}]",
            entries
                .iter()
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let _ = Command::new("gsettings")
            .args(&["set", schema_main, "custom-keybindings", &formatted_list])
            .status();
    }

    // Set the specific binding values
    let _ = Command::new("gsettings")
        .args(&[
            "set",
            &format!("{}:{}", schema_custom, target_path),
            "name",
            name_to_find,
        ])
        .status();
    let _ = Command::new("gsettings")
        .args(&[
            "set",
            &format!("{}:{}", schema_custom, target_path),
            "command",
            path_str,
        ])
        .status();
    let _ = Command::new("gsettings")
        .args(&[
            "set",
            &format!("{}:{}", schema_custom, target_path),
            "binding",
            binding,
        ])
        .status();

    // Especial para Ubuntu: Win+V abre el calendario por defecto.
    // Si el usuario elige Win+V, debemos deshabilitar la acción del shell.
    // Si elige otra cosa, nos aseguramos de que la del shell esté habilitada (si era Win+V)
    if binding == "<Super>v" {
        let _ = Command::new("gsettings")
            .args(&[
                "set",
                "org.gnome.shell.keybindings",
                "message-list-toggle",
                "[]",
            ])
            .status();
    } else {
        // Restaurar si cambiamos a otro atajo
        let _ = Command::new("gsettings")
            .args(&[
                "set",
                "org.gnome.shell.keybindings",
                "message-list-toggle",
                "['<Super>v']",
            ])
            .status();
    }
}

fn main() {
    glib::set_prgname(Some("io.github.klipbored.app"));
    glib::set_application_name("klipBored");

    // Register signal 2 for clean exit
    #[cfg(target_os = "linux")]
    glib::unix_signal_add(2, || {
        std::process::exit(0);
    });

    let app = adw::Application::builder()
        .application_id("io.github.klipbored.app")
        .flags(gio::ApplicationFlags::FLAGS_NONE)
        .build();

    app.connect_startup(|_| {
        load_css();
        // Register icon in the default icon theme so set_icon_name("klipbored") works
        if let Some(display) = gdk::Display::default() {
            let theme = gtk::IconTheme::for_display(&display);
            // Add the installed icon path
            let icon_dir = glib::home_dir().join(".local/share/icons");
            theme.add_search_path(icon_dir.to_str().unwrap_or_default());
        }
        gtk::Window::set_default_icon_name("io.github.klipbored.app");
    });

    app.connect_activate(move |app| {
        let windows = app.windows();
        let window = if let Some(w) = windows.first() {
            w.clone()
        } else {
            // Si por algún motivo no hay ventana, no hacemos nada en activate
            // Relm4 la creará en su momento.
            return;
        };

        if !has_keybinding() {
            window.set_visible(true);
            window.present();
            return;
        }

        if window.is_visible() && window.is_active() {
            // Si está visible Y tiene el foco, la ocultamos (toggle off)
            window.set_visible(false);
        } else {
            // Si está oculta O visible pero sin foco, la mostramos/traemos al frente (toggle on)
            window.set_visible(true);
            window.present();
        }
    });

    app.connect_window_added(move |_, window| {
        // Ocultar si pierde el foco
        let focus_controller = gtk::EventControllerFocus::new();
        let win_clone = window.clone();
        focus_controller.connect_leave(move |_| {
            // Un pequeño retardo para evitar parpadeos si el foco se mueve a un submenú o similar
            let w = win_clone.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                if !w.is_active() && w.is_visible() {
                    w.set_visible(false);
                }
                glib::ControlFlow::Break
            });
        });
        window.add_controller(focus_controller);
    });

    RelmApp::from_app(app).run::<KlipBoredModel>(());
}
