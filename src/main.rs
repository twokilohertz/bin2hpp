use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{ArgAction, Parser};

#[cfg(windows)]
const LINE_ENDING: &'static str = "\r\n";
#[cfg(not(windows))]
const LINE_ENDING: &'static str = "\n";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Input file path
    #[arg(short, long)]
    input_path: PathBuf,
    /// Output file path
    #[arg(short, long)]
    output_path: Option<PathBuf>,
    /// Name of the C++ symbol
    #[arg(short, long)]
    symbol_name: Option<String>,
    /// Namespace in which to put the symbol
    #[arg(short, long)]
    namespace: Option<String>,
    /// Whether to operate in binary mode as opposed to text mode (default: text mode)
    #[arg(short, long, action = ArgAction::SetTrue)]
    binary: Option<bool>,
}

fn main() -> ExitCode {
    let cli_args = CliArgs::parse();

    if !cli_args.input_path.exists() {
        eprintln!(
            "file path \"{}\" does not exist",
            cli_args.input_path.to_string_lossy()
        );
        return ExitCode::FAILURE;
    }

    if !cli_args.input_path.is_file() {
        eprintln!(
            "file path \"{}\" is not a file",
            cli_args.input_path.to_string_lossy()
        );
        return ExitCode::FAILURE;
    }

    // Derive output path from cwd & original filename if not provided in CLI

    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("environment's current working directory is unavailable");
            return ExitCode::FAILURE;
        }
    };

    let input_filename = match cli_args.input_path.file_name() {
        Some(f) => f,
        None => {
            eprintln!(
                "input file path \"{}\" does not contain a valid filename",
                cli_args.input_path.to_string_lossy()
            );
            return ExitCode::FAILURE;
        }
    };

    let output_path = match cli_args.output_path {
        Some(p) => p,
        None => cwd.join(input_filename).with_extension("hpp"),
    };

    let symbol_name = match cli_args.symbol_name {
        Some(s) => s,
        None => input_filename
            .to_string_lossy()
            .to_string()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect(),
    };

    let input_file = match OpenOptions::new().read(true).open(cli_args.input_path) {
        Ok(f) => f,
        Err(error) => {
            eprintln!("failed to open input file for reading: {}", error);
            return ExitCode::FAILURE;
        }
    };

    let buf = match read_file(&input_file) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("failed read input file: {}", error);
            return ExitCode::FAILURE;
        }
    };

    let formatted = match cli_args.binary {
        Some(true) => format_as_binary(&buf),
        _ => format_as_text(&buf),
    };

    let out_src = match cli_args.binary {
        Some(true) => {
            generate_src_for_array(&formatted, buf.len(), &symbol_name, cli_args.namespace)
        }
        _ => generate_src_for_string(&formatted, &symbol_name, cli_args.namespace),
    };

    let output_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)
    {
        Ok(f) => f,
        Err(error) => {
            eprintln!("failed to open output file for writing: {}", error);
            return ExitCode::FAILURE;
        }
    };

    let mut writer = BufWriter::new(output_file);
    match writer.write_all(out_src.as_bytes()) {
        Ok(_) => (),
        Err(error) => {
            eprintln!("failed to write to output file: {}", error);
            return ExitCode::FAILURE;
        }
    };

    return ExitCode::SUCCESS;
}

fn read_file(f: &File) -> io::Result<Vec<u8>> {
    let buf_size: u64 = match f.metadata() {
        Ok(metadata) => metadata.len(),
        Err(_) => 0x1000, // just preallocate 4 KiB otherwise
    };
    let mut buf: Vec<u8> = Vec::with_capacity(buf_size as usize);

    let mut reader = BufReader::new(f);
    reader.read_to_end(&mut buf)?;

    return Ok(buf);
}

/// Format a slice of bytes into an array-of-bytes initialiser list
fn format_as_binary(data: &[u8]) -> String {
    let mut formatted = data
        .iter()
        .map(|b| format!("{:#x},", b))
        .collect::<String>();
    if !formatted.is_empty() {
        formatted.pop().unwrap(); // remove trailing ','
    }

    return formatted;
}

/// Format a slice of bytes into a string literal (without quotes)
fn format_as_text(data: &[u8]) -> String {
    // FIXME: this will currently panic if the input file was not UTF-8 encoded!
    return String::from_utf8(data.to_vec())
        .unwrap()
        .escape_default()
        .collect();
}

fn generate_src_for_array(
    array_contents: &str,
    array_len: usize,
    symbol_name: &str,
    ns_name: Option<String>,
) -> String {
    // Includes
    let mut out_string: String = String::with_capacity(array_contents.len() + 0x100);
    out_string.push_str("#include <array>");
    out_string.push_str(LINE_ENDING);
    out_string.push_str("#include <cstdint>");
    out_string.push_str(LINE_ENDING);

    // Namespace
    match ns_name {
        Some(ref namespace) => out_string.push_str(format!("namespace {}{{", namespace).as_str()),
        None => (),
    };

    // Array declaration
    out_string.push_str(
        format!(
            "constexpr std::array<std::uint8_t,{}> {}{{{}}};",
            array_len, symbol_name, array_contents
        )
        .as_str(),
    );

    // Close namespace (if need be)
    match ns_name {
        Some(_) => out_string.push_str("}"),
        None => (),
    };

    // Trailing newline
    out_string.push_str(LINE_ENDING);

    return out_string;
}

fn generate_src_for_string(
    string_contents: &str,
    symbol_name: &str,
    ns_name: Option<String>,
) -> String {
    // Includes
    let mut out_string: String = String::with_capacity(string_contents.len() + 0x100);

    // Namespace
    match ns_name {
        Some(ref namespace) => out_string.push_str(format!("namespace {}{{", namespace).as_str()),
        None => (),
    };

    // String initialisation
    out_string.push_str(
        format!(
            "constexpr const char* {} = \"{}\";",
            symbol_name, string_contents
        )
        .as_str(),
    );

    // Close namespace (if need be)
    match ns_name {
        Some(_) => out_string.push_str("}"),
        None => (),
    };

    // Trailing newline
    out_string.push_str(LINE_ENDING);

    return out_string;
}
