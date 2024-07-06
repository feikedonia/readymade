use color_eyre::{eyre::eyre, Result};
use itertools::Itertools;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use crate::{
    backend::repart_output::{systemd_version, RepartOutput},
    stage,
    util::{self, LIVE_BASE},
};

const REPART_DIR: &str = "/usr/share/readymade/repart-cfgs/";

#[derive(Debug, Clone)]
pub enum InstallationType {
    WholeDisk,
    DualBoot(u64),
    ChromebookInstall,
    Custom,
}

#[tracing::instrument]
pub fn setup_system(output: RepartOutput) -> Result<()> {
    let mut container = output.to_container()?;

    // The reason we're checking for UEFI here is because we want to check the current
    // system's boot mode before we install GRUB, not check inside the container
    let uefi = util::check_uefi();
    container
        // first, upcast color_eyre::Report to a dyn Error
        .run(|| Ok(_inner_sys_setup(uefi, output)?))
        .map_err(|e| {
            // then, downcast back to color_ey::Report for better error diagnostics
            // SAFETY: _inner_sys_setup() returns error under color_eyre::Report.
            eyre!("Error configuring system")
                .wrap_err(unsafe { Box::from_raw(Box::into_raw(e).cast::<color_eyre::Report>()) })
        })?;

    Ok(())
}

#[tracing::instrument]
fn _inner_sys_setup(uefi: bool, output: RepartOutput) -> color_eyre::Result<()> {
    if uefi {
        // The reason why we don't do grub2-install here is because for
        // Fedora specifically, the install script simply plops in
        // a pre-built GRUB binary in the ESP that looks for the stage 1
        // config in /boot/efi/EFI/fedora/grub.cfg
        // The following config then redirects to the actual stage 2 config located
        // in /boot/grub2/grub.cfg
        // This is actually done to support BLS entries properly on their end

        // todo: Add support for systemd-boot
        std::fs::create_dir_all("/boot/efi/EFI/fedora")?;

        stage!("Generating stage 1 grub.cfg in ESP..." {
            let mut grub_cfg = std::fs::File::create("/boot/efi/EFI/fedora/grub.cfg")?;
            grub_cfg.write_all(crate::util::GRUB_CONFIG.as_bytes())?;
        });

        stage!("Generating stage 2 grub.cfg in /boot/grub2/grub.cfg..." {
            _ = std::process::Command::new("grub2-mkconfig")
                .arg("-o")
                .arg("/boot/grub2/grub.cfg")
                .status()?;
        });
    }

    stage!("Cleaning up /boot partition..." {
        let boot_dir = Path::new("/boot");
        for file in std::fs::read_dir(boot_dir)?.flatten().map(|entry| entry.path()) {
            let file_name = file.file_name().unwrap().to_str().unwrap();
            if file_name.starts_with("initramfs") || file_name.starts_with("vmlinuz") {
                tracing::debug!(?file, "Removing kernel file");
                std::fs::remove_file(file)?;
            }
        }

        let bls_dir = Path::new("/boot/loader/entries");
        for file in std::fs::read_dir(bls_dir)?.flatten().map(|entry| entry.path()) {
            tracing::debug!(?file, "Removing BLS entry");
            std::fs::remove_file(file)?;
        }
    });

    // Reinstall kernel
    //
    // Here we're going to reinstall the kernel with an initramfs optimized
    // for the new system configuration. We'll be doing this by using kernel-install
    //
    // which runs all the necessary hooks to generate the initramfs and install the kernel properly.
    //
    // As a bonus, it also generates the BLS entries for us.
    stage!("Reinstalling kernel" {
        // list all kernels in /lib/modules
        // suggestion: Switch to using kernel-install --json=short for parsing
        let kernel_vers = std::fs::read_dir("/lib/modules")?
            .map(|entry| entry.unwrap().file_name())
            .collect_vec();

        tracing::info!(?kernel_vers, "Kernel versions found");

        // We're gonna just install the first kernel we find, so let's do that
        let kver = kernel_vers.first().unwrap().to_str().unwrap();

        // install kernel

        std::process::Command::new("kernel-install")
            .arg("add")
            .arg(kver)
            .arg(format!("/lib/modules/{kver}/vmlinuz"))
            .arg("--verbose")
            .status()?;
    });
    // Generate /etc/fstab
    if systemd_version()? <= 256 {
        stage!("Generating /etc/fstab..." {
            let mut fstab = std::fs::File::create("/etc/fstab")?;
            fstab.write_all(output.into_fstab().as_bytes())?;
        });
    }

    stage!("Initializing system" {
        _initialize_system()?;
    });

    stage!("Setting SELinux contexts..." {
        std::process::Command::new("setfiles")
            .args(&["-e", "/proc", "-e", "/sys"])
            .arg("/etc/selinux/targeted/contexts/files/file_contexts")
            .arg("/")
            .status()?;
    });

    Ok(())
}

/// Initialize the system after installation
/// This function is moved to a separate function to allow for cleaner code
#[tracing::instrument]
fn _initialize_system() -> color_eyre::Result<()> {
    if std::fs::metadata("/var/lib/systemd/random-seed").is_ok() {
        std::fs::remove_file("/var/lib/systemd/random-seed")?;
    }

    if std::fs::metadata("/etc/machine-id").is_ok() {
        std::fs::remove_file("/etc/machine-id")?;
    }
    // We're gonna make an empty machine-id file so that systemd can generate a new one
    std::fs::File::create("/etc/machine-id")?;

    // wipe NetworkManager state
    if std::fs::metadata("/etc/NetworkManager/system-connections").is_ok() {
        for entry in std::fs::read_dir("/etc/NetworkManager/system-connections")? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
            }
        }
    }

    // todo: Copy over NetworkManager state from current livesys

    // wipe temporary RPM database
    if std::fs::metadata("/var/lib/rpm").is_ok() {
        for entry in std::fs::read_dir("/var/lib/rpm")? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("__db") {
                std::fs::remove_file(entry.path())?;
            }
        }
    }

    // wipe temporary dnf cache
    if std::fs::metadata("/var/cache/dnf").is_ok() {
        for entry in std::fs::read_dir("/var/cache/dnf")? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            }
        }
    }
    
    // todo: set locale and timezone from config

    Ok(())
}

impl InstallationType {
    #[tracing::instrument]
    pub fn install(&self, state: &crate::InstallationState) -> Result<()> {
        // SAFETY: ideally we should never reach this point without a valid destination disk
        let blockdev = &state
            .destination_disk
            .as_ref()
            .expect("A valid destination device should be set before calling install()")
            .devpath;
        let cfgdir = self.cfgdir();

        // todo: not freeze on error, show error message as err handler?
        let repart_out = Self::systemd_repart(blockdev, &cfgdir)?;
        tracing::info!("Copying files done, Setting up system...");
        setup_system(repart_out)?;

        if let Self::ChromebookInstall = self {
            Self::set_cgpt_flags(blockdev)?;
        }
        tracing::info!("install() finished");
        Ok(())
    }

    /// Mount a device or file to /mnt/live-base
    fn mount_dev(dev: &str) -> std::io::Result<sys_mount::Mount> {
        const MOUNTPOINT: &str = "/mnt/live-base";
        std::fs::create_dir_all(MOUNTPOINT)?;
        sys_mount::Mount::builder().mount(dev, MOUNTPOINT)
    }

    fn cfgdir(&self) -> PathBuf {
        match self {
            Self::ChromebookInstall => const_format::concatcp!(REPART_DIR, "chromebookinstall"),
            _ => todo!(),
        }
        .into()
    }

    // todo: Generate custom repart partitioning definitions in case the user wants to use a custom partitioning scheme
    #[tracing::instrument]
    fn systemd_repart(
        blockdev: &Path,
        cfgdir: &Path,
    ) -> Result<crate::backend::repart_output::RepartOutput> {
        let copy_source = {
            const FALLBACK: &str = "/mnt/live-base";
            // We'll be using a new feature from systemd 255 (relative repart copy source)
            // to copy the repartitioning definitions from the live base to the target disk

            // environment variable override. This is documented in HACKING.md

            if let Ok(copy_source) = std::env::var("REPART_COPY_SOURCE") {
                tracing::info!("Using REPART_COPY_SOURCE override: {copy_source}");
                let copy_source = Path::new(&copy_source.trim()).canonicalize()?;

                if copy_source == Path::new("/") {
                    tracing::warn!("REPART_COPY_SOURCE is set to `/`, this is likely a mistake. Copying entire host root filesystem to target disk...");
                }

                // convert back to string, may cause performance issues but it's not a big deal
                copy_source.to_string_lossy().to_string()
            }
            // if we can mount /dev/mapper/live-base, we'll use that as the copy source
            else {
                match Self::mount_dev(crate::util::LIVE_BASE) {
                    Ok(mount) => {
                        let m = mount.target_path().to_string_lossy().to_string();
                        tracing::info!("Mounted live-base at {m}");
                        m
                    }
                    Err(e) => {
                        tracing::error!("Failed to mount `{LIVE_BASE}`, using `{FALLBACK}` as copy source anyway... ({e})");
                        FALLBACK.to_string()
                    }
                }
            }
        };

        let arg = if systemd_version()? >= 256 {
            "--generate-fstab"
        } else {
            ""
        };
        
        let dry_run = if cfg!(debug_assertions) { "yes" } else { "no" };
        tracing::debug!(?dry_run, "Running systemd-repart");
        let out = cmd_lib::run_fun!(
            pkexec systemd-repart
                --dry-run=$dry_run
                --definitions=$cfgdir
                --empty=force
                $arg
                --copy-source=$copy_source
                --json=pretty
                $blockdev
        )
        .map_err(|e| color_eyre::eyre::eyre!("systemd-repart failed").wrap_err(e))?;

        // todo: wait for systemd 256 or genfstab magic
        tracing::debug!("systemd-repart finished");
        Ok(serde_json::from_str(&out)?)
    }
    fn set_cgpt_flags(blockdev: &Path) -> Result<()> {
        tracing::debug!("Setting cgpt flags");
        cmd_lib::run_cmd!(cgpt add -i 1 -t kernel -P 15 -T 1 -S 1 $blockdev)?;
        Ok(())
    }
}
