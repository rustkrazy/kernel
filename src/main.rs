use std::fs::File;
use std::path::Path;

const LATEST: &str = "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.1.1.tar.xz";

const CONFIG: &str = r#"
CONFIG_IPV6=y
"#;

fn download_kernel() -> anyhow::Result<()> {
    println!("Downloading kernel source...");

    let file_name = Path::new(LATEST).file_name().unwrap().to_str().unwrap();

    let mut file = File::create(file_name)?;

    reqwest::blocking::get(LATEST)?
        .error_for_status()?
        .copy_to(&mut file)?;

    println!("Kernel source downloaded successfully");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    download_kernel()?;

    Ok(())
}
