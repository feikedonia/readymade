use crate::{InstallationType, NavigationAction, Page, INSTALLATION_STATE};
use gtk::prelude::*;
use libhelium::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};

pub struct InstallationTypePage {}

#[derive(Debug)]
pub enum InstallationTypePageMsg {
    Update,
    #[doc(hidden)]
    Navigate(NavigationAction),
    InstallationTypeSelected(InstallationType),
}

#[derive(Debug)]
pub enum InstallationTypePageOutput {
    Navigate(NavigationAction),
}

#[relm4::component(pub)]
impl SimpleComponent for InstallationTypePage {
    type Init = ();
    type Input = InstallationTypePageMsg;
    type Output = InstallationTypePageOutput;

    view! {
        libhelium::ViewMono {
            set_title: "Installation Type",
            set_vexpand: true,

            add = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 4,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::Center,
                    set_spacing: 16,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 2,
                        set_vexpand: true,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,
                        set_halign: gtk::Align::Center,

                        gtk::Image {
                            set_icon_name: Some("drive-harddisk"),
                            inline_css: "-gtk-icon-size: 128px"
                        },

                        gtk::Label {
                            #[watch]
                            set_label: &INSTALLATION_STATE.read().destination_disk.clone().map(|d| d.disk_name).unwrap_or("".to_owned()),
                            inline_css: "font-size: 16px; font-weight: bold"
                        },

                        gtk::Label {
                            #[watch]
                            set_label: &INSTALLATION_STATE.read().destination_disk.clone().map(|d| d.os_name).unwrap_or("".to_owned()),
                        }
                    },

                    gtk::Box {
                        set_spacing: 8,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::End,
                        set_homogeneous: true,
                        libhelium::PillButton {
                            set_label: "Entire Disk",
                            inline_css: "padding-left: 48px; padding-right: 48px",
                            connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::WholeDisk)
                        },
                        libhelium::PillButton {
                            set_label: "Dual Boot",
                            inline_css: "padding-left: 48px; padding-right: 48px",
                            connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::DualBoot)
                        },
                        libhelium::PillButton {
                            set_label: "Custom",
                            inline_css: "padding-left: 48px; padding-right: 48px",
                            connect_clicked => InstallationTypePageMsg::InstallationTypeSelected(InstallationType::Custom)
                        }
                    }
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 4,

                    libhelium::TextButton {
                        set_label: "Previous",
                        connect_clicked => InstallationTypePageMsg::Navigate(NavigationAction::GoTo(crate::Page::Destination))
                    },

                    gtk::Box {
                        set_hexpand: true,
                    }
                }
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};

        let widgets = view_output!();

        INSTALLATION_STATE.subscribe(sender.input_sender(), |_| InstallationTypePageMsg::Update);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InstallationTypePageMsg::InstallationTypeSelected(InstallationType::WholeDisk) => {
                let mut installation_state_guard = INSTALLATION_STATE.write();
                installation_state_guard.installation_type = Some(InstallationType::WholeDisk);
                sender
                    .output(InstallationTypePageOutput::Navigate(
                        NavigationAction::GoTo(Page::Confirmation),
                    ))
                    .unwrap()
            }
            InstallationTypePageMsg::InstallationTypeSelected(InstallationType::DualBoot) => {}
            InstallationTypePageMsg::InstallationTypeSelected(InstallationType::Custom) => {}
            InstallationTypePageMsg::Navigate(action) => sender
                .output(InstallationTypePageOutput::Navigate(action))
                .unwrap(),
            InstallationTypePageMsg::Update => {}
        }
    }
}
