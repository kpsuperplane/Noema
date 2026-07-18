//! Export the GraphQL SDL for frontend code generation.

use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
enum Destination {
    Output(PathBuf),
    Stdout,
}

fn main() -> io::Result<()> {
    let destination = parse_destination(env::args_os().skip(1))?;
    export_schema(destination, &env::current_dir()?, &mut io::stdout().lock())
}

fn parse_destination(
    args: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
) -> io::Result<Destination> {
    let mut args = args.into_iter().map(Into::into);
    let Some(flag) = args.next() else {
        return Err(invalid_input(
            "missing destination; pass --output <path> or --stdout",
        ));
    };
    let destination = match flag.to_str() {
        Some("--output") => {
            let path = args
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid_input("--output requires a path"))?;
            Destination::Output(PathBuf::from(path))
        }
        Some("--stdout") => Destination::Stdout,
        _ => {
            return Err(invalid_input("expected --output <path> or --stdout"));
        }
    };
    if args.next().is_some() {
        return Err(invalid_input("unexpected additional argument"));
    }
    Ok(destination)
}

fn export_schema(destination: Destination, cwd: &Path, stdout: &mut impl Write) -> io::Result<()> {
    let sdl = noema_api::graphql::schema_sdl();
    match destination {
        Destination::Output(path) => {
            let output_path = if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            };
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, &sdl)?;
            writeln!(stdout, "wrote {}", output_path.display())
        }
        Destination::Stdout => stdout.write_all(sdl.as_bytes()),
    }
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
