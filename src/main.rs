use anyhow::bail;
use clap::Parser;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::Path;
use std::process::{Command, Stdio};

const LATEST: &str = "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.1.1.tar.xz";

const CONFIG: &str = r#"
CONFIG_SQUASHFS=y
CONFIG_SQUASHFS_FILE_CACHE=y
CONFIG_SQUASHFS_DECOMP_MULTI_PERCPU=y
CONFIG_SQUASHFS_ZSTD=y
CONFIG_RASPBERRYPI_FIRMWARE=y
CONFIG_RASPBERRYPI_POWER=y
CONFIG_TUN=y
"#;

#[derive(Debug, Parser)]
#[command(author = "The Rustkrazy Authors", version = "v0.1.0", about = "Build the rustkrazy kernel", long_about = None)]
struct Args {
    /// Output architecture.
    #[arg(short = 'a', long = "architecture")]
    arch: String,
}

fn download_kernel(file_name: &str) -> anyhow::Result<()> {
    println!("Downloading kernel source...");

    let mut file = File::create(file_name)?;

    reqwest::blocking::get(LATEST)?
        .error_for_status()?
        .copy_to(&mut file)?;

    println!("Kernel source downloaded successfully");
    Ok(())
}

fn compile(arch: &str, cross: Option<String>, img: &str) -> anyhow::Result<()> {
    let arch_arg = format!("ARCH={}", arch);
    let cross_arg = cross.map(|v| format!("CROSS_COMPILE={}", v));

    let mut defconfig = no_stdin("make");
    defconfig.arg(&arch_arg).arg("defconfig");

    if !defconfig.spawn()?.wait()?.success() {
        bail!("make defconfig failed");
    }

    let mut mod2noconfig = no_stdin("make");
    mod2noconfig.arg(&arch_arg).arg("mod2noconfig");

    if !mod2noconfig.spawn()?.wait()?.success() {
        bail!("make mod2noconfig failed");
    }

    // Drop and close the file before continuing.
    {
        let mut file = File::options()
            .truncate(false)
            .append(true)
            .open(".config")?;

        file.write_all(CONFIG.as_bytes())?;
    }

    let mut olddefconfig = no_stdin("make");
    olddefconfig.arg(&arch_arg).arg("olddefconfig");

    if !olddefconfig.spawn()?.wait()?.success() {
        bail!("make olddefconfig failed");
    }

    let mut make = no_stdin("make");
    make.arg(&arch_arg);

    if let Some(cross_compile) = cross_arg {
        make.arg(cross_compile);
    }

    make.arg(img)
        .arg("modules")
        .arg("-j".to_owned() + &num_cpus::get().to_string());

    if !make.spawn()?.wait()?.success() {
        bail!("make failed");
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let arch = String::from(match args.arch.as_str() {
        "x86_64" => "x86_64",
        "rpi" => "arm64",
        _ => bail!("invalid architecture (supported: x86_64 rpi)"),
    });

    let cross = match args.arch.as_str() {
        "x86_64" => None,
        "rpi" => Some(String::from("aarch64-linux-gnu-")),
        _ => bail!("invalid architecture (supported: x86_64 rpi)"),
    };

    let img = String::from(match args.arch.as_str() {
        "x86_64" => "bzImage",
        "rpi" => "Image.gz",
        _ => bail!("invalid architecture (supported: x86_64 rpi)"),
    });

    let file_name = Path::new(LATEST).file_name().unwrap().to_str().unwrap();

    download_kernel(file_name)?;

    let mut untar = no_stdin("tar");
    untar.arg("xf").arg(file_name);

    if !untar.spawn()?.wait()?.success() {
        bail!("untar failed");
    }

    println!("Kernel source unpacked successfully");

    let current_dir = env::current_dir()?;
    env::set_current_dir(file_name.trim_end_matches(".tar.xz"))?;

    println!("Compiling kernel...");
    compile(&arch, cross, &img)?;
    println!("Kernel compiled successfully");

    let kernel_path = format!("arch/{}/boot/{}", arch, img);

    env::set_current_dir(current_dir)?;

    fs::copy(
        Path::new(file_name.trim_end_matches(".tar.xz")).join(kernel_path),
        format!("vmlinuz-{}", args.arch),
    )?;

    fs::remove_file(file_name)?;
    fs::remove_dir_all(file_name.trim_end_matches(".tar.xz"))?;

    Ok(())
}

fn no_stdin<S: AsRef<OsStr>>(program: S) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());

    cmd
}
