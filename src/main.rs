use anyhow::bail;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

const LATEST: &str = "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.1.1.tar.xz";

const CONFIG: &str = r#"
CONFIG_SQUASHFS_DECOMP_MULTI_PERCPU=y
CONFIG_IPV6=y
"#;

fn download_kernel(file_name: &str) -> anyhow::Result<()> {
    println!("Downloading kernel source...");

    let mut file = File::create(file_name)?;

    reqwest::blocking::get(LATEST)?
        .error_for_status()?
        .copy_to(&mut file)?;

    println!("Kernel source downloaded successfully");
    Ok(())
}

fn compile() -> anyhow::Result<()> {
    let mut defconfig = Command::new("make");
    defconfig.arg("defconfig");

    if !defconfig.spawn()?.wait()?.success() {
        bail!("make defconfig failed");
    }

    let mut mod2noconfig = Command::new("make");
    mod2noconfig.arg("mod2noconfig");

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

    let mut olddefconfig = Command::new("make");
    olddefconfig.arg("olddefconfig");

    if !olddefconfig.spawn()?.wait()?.success() {
        bail!("make olddefconfig failed");
    }

    let mut make = Command::new("make");
    make.arg("bzImage")
        .arg("modules")
        .arg("-j".to_owned() + &num_cpus::get().to_string());

    if !make.spawn()?.wait()?.success() {
        bail!("make failed");
    }

    Ok(())
}

fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> anyhow::Result<()> {
    let mut outfile = File::create(dst)?;
    let mut infile = File::open(src)?;

    let mut buf = Vec::new();
    infile.read_to_end(&mut buf)?;
    outfile.write_all(&buf)?;

    outfile.set_permissions(infile.metadata()?.permissions())?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let file_name = Path::new(LATEST).file_name().unwrap().to_str().unwrap();

    download_kernel(file_name)?;

    let mut untar = Command::new("tar");
    untar.arg("xf").arg(file_name);

    if !untar.spawn()?.wait()?.success() {
        bail!("untar failed");
    }

    println!("Kernel source unpacked successfully");

    let current_dir = env::current_dir()?;
    env::set_current_dir(file_name.trim_end_matches(".tar.xz"))?;

    println!("Compiling kernel...");
    compile()?;
    println!("Kernel compiled successfully");

    let kernel_path = "arch/x86_64/boot/bzImage"; /* FIXME: arch independent */

    env::set_current_dir(current_dir)?;

    copy_file(
        Path::new(file_name.trim_end_matches(".tar.xz")).join(kernel_path),
        "vmlinuz",
    )?;

    fs::remove_file(file_name)?;
    fs::remove_dir_all(file_name.trim_end_matches(".tar.xz"))?;

    Ok(())
}
