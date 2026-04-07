//! Extract and check Path from Windows Registry.
//!
//! Windows Registry is the ultimate source of truth for environment variables.
//! However, instead of storing the variables directly, the registry can contain
//! `%`-expansions that need to be resolved.
//!
//! Also, we can ask the user to update the environment with an editor.
use std::borrow::Cow;
use std::env::{join_paths, split_paths, var_os};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::iter::once;
use std::os::windows::ffi::OsStrExt as _;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;
use std::{fs, slice};

use indexmap::IndexMap;
use indexmap::map::Entry;
use regex::bytes::{Captures, Regex};
use tempfile::NamedTempFile;
use winreg::enums::RegType;
use winreg::types::FromRegValue;
use winreg::{RegKey, RegValue};

/// Expands environment variables with syntax %VAR% recursively
pub fn expand_env(env: &OsStr, reg_env: &RegKey) -> OsString {
    let mut env = Cow::from(env);
    while env.as_encoded_bytes().contains(&b'%') {
        static ENV_VAR_REGEX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"%([A-Za-z0-9_]+)%").unwrap());

        let original = env.as_encoded_bytes();
        let expanded = ENV_VAR_REGEX.replace_all(original, |caps: &Captures| {
            let name = unsafe { OsStr::from_encoded_bytes_unchecked(&caps[1]) };
            reg_env
                .get_value::<OsString, _>(name)
                .ok()
                .or_else(|| var_os(name))
                .unwrap_or_default()
                .as_encoded_bytes()
                .to_vec()
        });
        if expanded == original {
            break;
        }
        env = unsafe { OsString::from_encoded_bytes_unchecked(expanded.into_owned()) }.into();
    }
    env.into_owned()
}

pub fn check_reg_paths(reg_env: RegKey) -> anyhow::Result<()> {
    let raw_path = OsString::from_reg_value(&reg_env.get_raw_value("Path")?)?;
    let mut has_invalid = false;
    let mut paths = IndexMap::<OsString, OsString>::new();
    for path in split_paths(&raw_path).map(|p| p.into_os_string()) {
        // eprintln!("Checking {}", path.display());
        let expanded = expand_env(&path, &reg_env);
        let expanded_path = Path::new(&expanded);
        if !(expanded.is_empty() || expanded_path.is_absolute() && expanded_path.is_dir()) {
            eprintln!("Possibly invalid PATH entry: {}", expanded_path.display());
            has_invalid = true;
            let mut out_path = OsString::from("# ");
            out_path.push(path);
            paths.insert(expanded, out_path);
        } else {
            match paths.entry(expanded) {
                Entry::Occupied(entry) => {
                    let prev = entry.get();
                    eprintln!("Path {prev:?} already exists while expanding from {path:?}");
                    has_invalid = true;
                }
                Entry::Vacant(entry) => {
                    entry.insert(path);
                }
            }
        }
    }
    if has_invalid {
        eprintln!("Find invalid Path item(s), opening with text editor.");

        // Prepare path file
        let mut file = NamedTempFile::new()?;
        let mut input_file = OsString::new();
        for path in paths.values() {
            input_file.push(path);
            input_file.push("\n");
        }

        file.as_file_mut()
            .write_all(input_file.as_encoded_bytes())?;
        let path = file.into_temp_path();

        for editor in [Editor::Zed, Editor::Code, Editor::Codium] {
            match editor.edit(&path) {
                Ok(output) => {
                    let value = join_paths(output.lines().filter(|p| !p.is_empty()))?;
                    reg_env.set_raw_value(
                        "Path",
                        &RegValue {
                            bytes: to_reg_bytes(&value),
                            vtype: RegType::REG_EXPAND_SZ,
                        },
                    )?;
                    break;
                }
                Err(e) => {
                    eprintln!("Failed to open with {editor:?}: {e:?}");
                }
            }
        }
    }

    Ok(())
}

fn to_reg_bytes(value: &OsString) -> Cow<'_, [u8]> {
    let v: Vec<u16> = value.encode_wide().chain(once(0)).collect();
    unsafe { slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2) }.into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Editor {
    /// Zed editor
    ///
    /// # Example
    ///
    /// ```cmd
    /// Zed.exe --wait file.txt
    /// ```
    Zed,
    /// Visual Studio Code
    ///
    /// # Example
    ///
    /// ```cmd
    /// code.cmd --wait file.txt
    /// ```
    Code,
    /// VSCodium
    ///
    /// # Example
    ///
    /// ```cmd
    /// codium.cmd --wait file.txt
    /// ```
    Codium,
}

impl Editor {
    /// Open a given file with
    pub fn edit(&self, path: &Path) -> anyhow::Result<String> {
        match self {
            Editor::Zed => Command::new("Zed.exe")
                .arg("--wait")
                .arg(path)
                .spawn()?
                .wait()?,
            Editor::Code => Command::new("code.cmd")
                .arg("--wait")
                .arg(path)
                .spawn()?
                .wait()?,
            Editor::Codium => Command::new("codium.cmd")
                .arg("--wait")
                .arg(path)
                .spawn()?
                .wait()?,
            // _ => unimplemented!(),
        };
        // Read from updated file, rejecting any invalid UTF-8.
        Ok(fs::read_to_string(path)?)
    }
}
