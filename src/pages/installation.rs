use crate::prelude::*;
use crate::{NavigationAction, INSTALLATION_STATE};
use color_eyre::Result;
use relm4::{Component, ComponentParts, ComponentSender};
use std::time::Duration;

#[relm4::widget_template(pub)]
impl WidgetTemplate for BentoCard {
    view! {
        gtk::Box {
            set_vexpand: true,
            set_hexpand: true,
            set_orientation: gtk::Orientation::Vertical,

            inline_css: "border-radius: 16px; background: url('file:///home/lea/Downloads/viewports-light.png') no-repeat center; background-size: cover",

            gtk::Box {
                set_spacing: 4,
                set_halign: gtk::Align::Fill,
                set_valign: gtk::Align::End,
                set_vexpand: true,

                add_css_class: "content-block",
                inline_css: "border-top-left-radius: 0px; border-top-right-radius: 0px;",

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,
                    set_halign: gtk::Align::Start,

                    #[name = "icon"]
                    gtk::Image {
                        set_halign: gtk::Align::Start,
                        set_icon_name: Some("dialog-question-symbolic"),
                        inline_css: "-gtk-icon-size: 28px",
                        set_margin_bottom: 8,
                    },
                    #[name = "title"]
                    gtk::Label {
                        set_halign: gtk::Align::Start,
                        inline_css: "font-weight: 600; font-size: 16px"
                    },
                    #[name = "description"]
                    gtk::Label {
                        set_halign: gtk::Align::Start,
                    }
                },

                gtk::Image {
                    set_valign: gtk::Align::End,
                    set_halign: gtk::Align::End,
                    set_icon_name: Some("go-next-symbolic"),
                    inline_css: "-gtk-icon-size: 28px",
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct InstallationPage {
    progress_bar: gtk::ProgressBar,
}

#[derive(Debug)]
pub enum InstallationPageMsg {
    StartInstallation,
    #[doc(hidden)]
    Navigate(NavigationAction),
    Update,
    #[doc(hidden)]
    Throb,
}

#[derive(Debug)]
pub enum InstallationPageCommandMsg {
    FinishInstallation(Result<()>),
}

#[derive(Debug)]
pub enum InstallationPageOutput {
    Navigate(NavigationAction),
    SendErr(String),
}

#[relm4::component(pub)]
impl Component for InstallationPage {
    type Init = ();
    type Input = InstallationPageMsg;
    type Output = InstallationPageOutput;
    type CommandOutput = InstallationPageCommandMsg;

    view! {
        libhelium::ViewMono {
            #[wrap(Some)]
            set_title = &gtk::Label {
                #[watch]
                set_label: &gettext("Installation"),
                set_css_classes: &["view-title"]
            },
            set_vexpand: true,

            append = &gtk::Box {
                set_hexpand: true,
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                // gtk::Box {
                //     set_vexpand: true,
                //     gtk::Label {
                //         set_label: "Some sort of ad/feature thing here idk."
                //     },
                // },

                gtk::Grid {
                    set_vexpand: true,
                    set_hexpand: true,
                    set_row_spacing: 8,
                    set_column_spacing: 8,

                    #[template]
                    attach[0, 0, 1, 2] = &BentoCard {
                        #[template_child]
                        icon {
                            set_icon_name: Some("explore-symbolic"),
                        },
                        #[template_child]
                        title {
                            set_label: &gettext("Welcome to Ultramarine Linux!"),
                        },
                        #[template_child]
                        description {
                            set_label: &gettext("Get to know your new operating system."),
                        }
                    },
                    #[template]
                    attach[0, 2, 1, 2] = &BentoCard {
                        #[template_child]
                        icon {
                            set_icon_name: Some("applications-development-symbolic"),
                        },
                        #[template_child]
                        title {
                            set_label: &gettext("Contribute to Ultramarine"),
                        },
                        #[template_child]
                        description {
                            set_label: &gettext("Learn how to contribute your time, money, or hardware."),
                        }
                    },
                    #[template]
                    attach[1, 0, 1, 3] = &BentoCard {
                        #[template_child]
                        icon {
                            set_icon_name: Some("chat-symbolic"),
                        },
                        #[template_child]
                        title {
                            set_label: &gettext("Need help?"),
                        },
                        #[template_child]
                        description {
                            set_label: &gettext("Ask in one of our chats!"),
                        }
                    },
                    #[template]
                    attach[1, 3, 1, 1] = &BentoCard {
                        #[template_child]
                        icon {
                            set_icon_name: Some("power-profile-power-saver-symbolic"),
                        },
                        #[template_child]
                        title {
                            set_label: &gettext("Sponsor Fyra Labs"),
                        },
                        #[template_child]
                        description {
                            set_label: &gettext("Sponsorships help us ship better software, faster."),
                        }
                    },
                },

                gtk::Label {
                    #[watch]
                    set_label: &*gettext("Installing base system...")
                },

                #[local_ref]
                progress_bar -> gtk::ProgressBar {}
            }
        }
    }

    fn init(
        _init: Self::Init, // TODO: use selection state saved in root
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self::default();
        let progress_bar = &model.progress_bar;

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationPageMsg::Update);

        gtk::glib::timeout_add(Duration::from_secs(1), move || {
            sender.input(InstallationPageMsg::Throb);
            gtk::glib::ControlFlow::Continue
        }); // TODO: cleanup

        ComponentParts { model, widgets }
    }

    #[tracing::instrument]
    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match message {
            InstallationPageMsg::StartInstallation => {
                sender.spawn_oneshot_command(|| {
                    let state = INSTALLATION_STATE.read();
                    tracing::debug!(?state, "Starting installation...");
                    InstallationPageCommandMsg::FinishInstallation(state.install_using_subprocess())
                });
            }
            InstallationPageMsg::Navigate(action) => sender
                .output(InstallationPageOutput::Navigate(action))
                .unwrap(),
            InstallationPageMsg::Update => {}
            InstallationPageMsg::Throb => self.progress_bar.pulse(),
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        match message {
            InstallationPageCommandMsg::FinishInstallation(res) => {
                tracing::debug!("Installation complete");
                if let Err(e) = res {
                    tracing::error!("Installation failed: {e:?}");
                    sender
                        .output(InstallationPageOutput::SendErr(format!("{e:?}")))
                        .unwrap();
                    sender
                        .output(InstallationPageOutput::Navigate(NavigationAction::GoTo(
                            crate::Page::Failure,
                        )))
                        .unwrap();
                } else {
                    sender
                        .output(InstallationPageOutput::Navigate(NavigationAction::GoTo(
                            crate::Page::Completed,
                        )))
                        .unwrap();
                }
            }
        }
    }
}
