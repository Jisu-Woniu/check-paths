use std::env::{split_paths, var_os};
use std::ffi::{OsStr, OsString};
use std::sync::LazyLock;

use itertools::Itertools;
use regex::bytes::{Captures, Regex};

fn main() {
    let mut ok = true;
    eprintln!("Checking PATH entries from environment variable...");
    let session_path = var_os("PATH").unwrap_or_default();
    ok &= check_paths(session_path);

    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

        eprintln!("Checking user PATH entries from registry...");
        // Expand environment variables with syntax %VAR% recursively until no more variables are found.
        let user_path = expand_env(
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey("Environment")
                .and_then(|k| k.get_value("Path"))
                .unwrap_or_default(),
        );

        ok &= check_paths(user_path);

        eprintln!("Checking system PATH entries from registry...");
        let system_path = expand_env(
            RegKey::predef(HKEY_LOCAL_MACHINE)
                .open_subkey("SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment")
                .and_then(|k| k.get_value("Path"))
                .unwrap_or_default(),
        );

        ok &= check_paths(system_path);
    }

    if ok {
        println!("All your PATH entries are valid.");
    }
}

/// Expands environment variables with syntax %VAR% recursively
fn expand_env(mut env: OsString) -> OsString {
    while env.as_encoded_bytes().contains(&b'%') {
        static ENV_VAR_REGEX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"%([A-Za-z0-9_]+)%").unwrap());

        let original = env.as_encoded_bytes();
        let expanded = ENV_VAR_REGEX.replace_all(original, |caps: &Captures| {
            var_os(unsafe { OsStr::from_encoded_bytes_unchecked(&caps[1]) })
                .unwrap_or_default()
                .as_encoded_bytes()
                .to_vec()
        });
        if expanded == original {
            break;
        }
        env = unsafe { OsString::from_encoded_bytes_unchecked(expanded.into_owned()) };
    }
    env
}

fn check_paths(session_path: OsString) -> bool {
    let mut ok = true;
    for path in split_paths(&session_path)
        .filter(|p| !p.as_os_str().is_empty() && !p.is_dir())
        .sorted_unstable()
    {
        eprintln!("Possibly invalid PATH entry: {}", path.display());
        ok = false;
    }
    ok
}
