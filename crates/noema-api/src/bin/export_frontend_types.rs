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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_missing_destination() {
        let error = parse_destination(Vec::<String>::new()).expect_err("missing destination");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("--output <path> or --stdout"));
    }

    #[test]
    fn writes_an_explicit_relative_output_beneath_the_callers_directory() {
        let cwd = tempfile::tempdir().expect("temporary caller directory");
        let mut output = Vec::new();

        export_schema(
            Destination::Output(PathBuf::from("generated/schema.graphql")),
            cwd.path(),
            &mut output,
        )
        .expect("export schema");

        let schema = fs::read_to_string(cwd.path().join("generated/schema.graphql"))
            .expect("read generated schema");
        assert!(schema.contains("type QueryRoot"));
        assert!(
            String::from_utf8(output)
                .expect("UTF-8")
                .contains("generated/schema.graphql")
        );
    }

    #[test]
    fn stdout_is_an_explicit_destination() {
        let cwd = tempfile::tempdir().expect("temporary caller directory");
        let mut output = Vec::new();

        export_schema(Destination::Stdout, cwd.path(), &mut output).expect("export schema");

        assert!(
            String::from_utf8(output)
                .expect("UTF-8")
                .contains("type QueryRoot")
        );
    }
}
