use std::env::{split_paths, var_os};

use itertools::Itertools;

fn main() {
    let mut ok = true;
    for path in split_paths(&var_os("PATH").unwrap_or_default())
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| (p.canonicalize(), p))
        .filter(|(res, _)| res.is_err())
        .sorted_unstable_by(|(_, p1), (_, p2)| p1.cmp(p2))
    {
        eprintln!(
            "Possibly invalid PATH entry: {}\n{}",
            path.1.display(),
            path.0.unwrap_err(),
        );
        ok = false;
    }
    if ok {
        println!("All your PATH entries are valid.");
    }
}
