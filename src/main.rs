#![warn(rust_2018_idioms)]
mod backend;
pub mod cfg;
mod consts;
mod disks;
mod install;
mod pages;
pub mod prelude;
mod util;

use crate::prelude::*;
use color_eyre::Result;
use gtk::glib::translate::FromGlibPtrNone;
use install::{InstallationState, InstallationType};
use pages::installation::InstallationPageMsg;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, RelmApp, SharedState,
    SimpleComponent,
};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

/// State related to the user's installation configuration
static INSTALLATION_STATE: SharedState<InstallationState> = SharedState::new();
static CONFIG: SharedState<cfg::ReadymadeConfig> = SharedState::new();
static ROOT_WINDOW_ID: SharedState<u32> = SharedState::new();

// todo: lazy_static const variables for the setup params

const APPID: &str = "com.fyralabs.Readymade";

macro_rules! generate_pages {
    ($Page:ident $AppModel:ident $AppMsg:ident: $($page:ident $($forward:expr)?),+$(,)?) => {paste::paste! {
        use pages::{$([<$page:lower>]::[<$page:camel Page>]),+};
        use pages::{$([<$page:lower>]::[<$page:camel PageOutput>]),+};


        #[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
        pub enum $Page {
            #[default]
            $([< $page:camel >]),+
        }

        struct $AppModel {
            page: $Page,
            $(
                [<$page:snake _page>]: relm4::Controller<[<$page:camel Page>]>,
            )+
        }

        impl $AppModel {
            fn _default(sender: ComponentSender<Self>) -> Self {Self {
                page: $Page::default(),
                $(
                    [<$page:snake _page>]: [<$page:camel Page>]::builder()
                        .launch(())
                        .forward(sender.input_sender(), generate_pages!(@$page $AppMsg $($forward)?)),
                )+
            }}
        }
    }};
    (@$page:ident $AppMsg:ident) => {paste::paste! {
        |msg| match msg {
            [<$page:camel PageOutput>]::Navigate(action) => $AppMsg::Navigate(action),
        }
    }};
    (@$page:ident $AppMsg:ident $forward:expr) => { $forward };
}

generate_pages!(Page AppModel AppMsg:
    Language,
    Welcome,
    Destination,
    InstallationType,
    Confirmation |msg| {
        tracing::debug!("ConfirmationPage emitted {msg:?}");
        match msg {
            ConfirmationPageOutput::StartInstallation => AppMsg::StartInstallation,
            ConfirmationPageOutput::Navigate(action) => AppMsg::Navigate(action),
        }
    },
    Installation |msg| {
        tracing::debug!("InstallationPage emitted {msg:?}");
        match msg {
            InstallationPageOutput::Navigate(action) => AppMsg::Navigate(action),
            InstallationPageOutput::SendErr(s) => AppMsg::SendErr(s),
        }
    },
    InstallDual,
    InstallCustom,
    Completed,
    Failure,
);

#[derive(Debug)]
pub enum NavigationAction {
    GoTo(Page),
    Quit,
}

#[derive(Debug)]
enum AppMsg {
    StartInstallation,
    Navigate(NavigationAction),
    SendErr(String),
}

#[allow(clippy::str_to_string)]
#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();

    type Input = AppMsg;
    type Output = ();

    view! {
        libhelium::ApplicationWindow {
            set_title: Some("Readymade Installer"),
            set_default_width: 550,
            set_default_height: 600,
            set_vexpand: true,

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_vexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                #[transition = "SlideLeftRight"]
                match model.page {
                    Page::Language => *model.language_page.widget(),
                    Page::Welcome => *model.welcome_page.widget(),
                    Page::Destination => *model.destination_page.widget(),
                    Page::InstallationType => *model.installation_type_page.widget(),
                    Page::Confirmation => *model.confirmation_page.widget(),
                    Page::Installation => *model.installation_page.widget(),
                    Page::InstallDual => *model.install_dual_page.widget(),
                    Page::InstallCustom => *model.install_custom_page.widget(),
                    Page::Completed => *model.completed_page.widget(),
                    Page::Failure => *model.failure_page.widget(),
                }
            }
        }
    }

    // Initialize the UI.
    fn init(
        (): Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // TODO: make libhelium force this
        let display = gtk::gdk::Display::default().unwrap();
        let settings = gtk::Settings::for_display(&display);
        *ROOT_WINDOW_ID.write() = root.id();

        initialize_custom_icons(display);
        settings.set_gtk_icon_theme_name(Some("Hydrogen"));

        let model = Self::_default(sender);

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::StartInstallation => {
                let value = INSTALLATION_STATE.read().installation_type;
                if let Some(InstallationType::Custom) = value {
                    INSTALLATION_STATE.write().mounttags =
                        Some(crate::backend::custom::MountTargets(
                            self.install_custom_page
                                .model()
                                .choose_mount_factory
                                .iter()
                                .cloned()
                                .collect(),
                        ));
                }
                self.installation_page
                    .emit(InstallationPageMsg::StartInstallation);
            }
            AppMsg::Navigate(NavigationAction::GoTo(page)) => self.page = page,

            // FIXME: The following code is commented out because it'd trigger relm4 to drop the
            // RegionPage and LanguagePage, and somehow during quitting, it triggers the signal
            // `:selected_children_changed` which causes the program to crash upon accessing the
            // dropped components.
            //
            // AppMsg::Navigate(NavigationAction::Quit) => relm4::main_application().quit(),
            AppMsg::Navigate(NavigationAction::Quit) => std::process::exit(0),
            AppMsg::SendErr(s) => self
                .failure_page
                .emit(pages::failure::FailurePageMsg::Err(s)),
        }
    }
}
// todo: non-interactive mode?
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
fn main() -> Result<()> {
    let _guard = setup_logs_and_install_panic_hook();

    if std::env::args().any(|arg| arg == "--non-interactive") {
        tracing::info!("Running in non-interactive mode");
        // Get installation state from stdin json instead

        let install_state: InstallationState = serde_json::from_reader(std::io::stdin())?;

        return install_state.install();
    }

    *CONFIG.write() = cfg::get_cfg()?;

    gettextrs::textdomain(APPID)?;
    gettextrs::bind_textdomain_codeset(APPID, "UTF-8")?;

    let app = libhelium::Application::builder()
        .application_id(APPID)
        .flags(libhelium::gtk::gio::ApplicationFlags::default())
        // SAFETY: placeholder
        .default_accent_color(unsafe {
            &libhelium::RGBColor::from_glib_none(std::ptr::from_mut(
                &mut libhelium::ffi::HeRGBColor {
                    r: 0.0,
                    g: 7.0,
                    b: 143.0,
                },
            ))
        })
        .build();

    tracing::debug!("Starting Readymade");
    RelmApp::from_app(app).run::<AppModel>(());
    Ok(())
}

fn initialize_custom_icons(display: gtk::gdk::Display) {
    gtk::gio::resources_register_include!("icons.gresource").unwrap();

    let theme = gtk::IconTheme::for_display(&display);
    theme.add_resource_path("/com/FyraLabs/Readymade/icons");
}

/// Returns a logging guard.
///
/// # Panics
/// - cannot install `color_eyre`
/// - cannot create readymade tempdir
#[allow(clippy::cognitive_complexity)]
fn setup_logs_and_install_panic_hook() -> impl std::any::Any {
    color_eyre::install().expect("install color_eyre");
    let temp_dir = tempfile::Builder::new()
        .prefix("readymade-logs")
        .tempdir()
        .expect("create readymade logs tempdir")
        .into_path();
    // create dir
    std::fs::create_dir_all(&temp_dir).expect("create readymade logs tempdir");
    let file_appender = tracing_appender::rolling::never(&temp_dir, "readymade.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer().pretty())
        .with(EnvFilter::from_env("READYMADE_LOG"))
        .with(
            tracing_journald::layer()
                .unwrap()
                .with_syslog_identifier("readymade".to_owned()),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .compact(),
        )
        .init();

    if cfg!(debug_assertions) {
        tracing::info!("Running in debug mode");
    }
    tracing::info!(
        "Readymade Installer {version}",
        version = env!("CARGO_PKG_VERSION")
    );
    tracing::info!("Logging to journald");
    tracing::info!(
        "Logging to {tmp}/readymade.log",
        tmp = temp_dir.to_string_lossy()
    );
    guard
}
