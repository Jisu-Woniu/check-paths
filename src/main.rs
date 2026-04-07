use std::env::{split_paths, var_os};
use std::ffi::OsString;

use itertools::Itertools;

#[cfg(windows)]
mod registry;

fn main() -> anyhow::Result<()> {
    let mut ok = true;
    eprintln!("Checking PATH entries from environment variable...");
    let session_path = var_os("PATH").unwrap_or_default();
    ok &= check_paths(session_path);

    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};

        eprintln!("Checking user PATH entries from registry...");
        // Expand environment variables with syntax %VAR% recursively until no more
        // variables are found.
        let user_reg_env = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("Environment", KEY_READ | KEY_SET_VALUE)
            .expect("Failed to open user's registry entries");

        registry::check_reg_paths(user_reg_env)?;

        // let user_path = expand_env(user_path_raw);

        // ok &= check_paths(user_path);

        eprintln!("Checking system PATH entries from registry...");
        let system_reg_env = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(
                "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                KEY_READ | KEY_SET_VALUE,
            )
            .or_else(|_| {
                RegKey::predef(HKEY_LOCAL_MACHINE)
                    .open_subkey("SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment")
            })?;

        registry::check_reg_paths(system_reg_env)?;

        // ok &= check_paths(system_path);
    }

    if ok {
        println!("All your PATH entries are valid.");
    }
    Ok(())
}

fn check_paths(session_path: OsString) -> bool {
    let mut ok = true;
    for (i, path) in split_paths(&session_path)
        .enumerate()
        .filter(|(_, p)| !(p.as_os_str().is_empty() || p.is_absolute() && p.is_dir()))
        .sorted_unstable_by_key(|(_, p)| p.clone())
    {
        eprintln!("Possibly invalid PATH entry {i}: {}", path.display());
        ok = false;
    }
    ok
}
