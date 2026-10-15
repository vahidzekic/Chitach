//! Chitach command-line interface.

mod args;
mod logger;

use std::io::ErrorKind;
use std::path::Path;
use std::process::ExitCode;

use celik_core::reader::{list_readers, PcscCard};
use celik_core::{read_document, CardTransport, Document};
use celik_export::PdfOptions;

use args::Config;

/// Invalid command line (configuration) error.
const EXIT_CONFIG: u8 = 1;
/// Error while reading the card or writing output.
const EXIT_RUNTIME: u8 = 2;

const REPOSITORY: &str = "https://github.com/vahidzekic/Chitach";

fn main() -> ExitCode {
    let cfg = match args::parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("Run 'chitach-cli --help' for usage.");
            return ExitCode::from(EXIT_CONFIG);
        }
    };

    if cfg.help {
        print!("{}", args::USAGE);
        return ExitCode::SUCCESS;
    }

    if cfg.version {
        println!("chitach-cli {}", env!("CARGO_PKG_VERSION"));
        println!("{REPOSITORY}");
        return ExitCode::SUCCESS;
    }

    logger::init(cfg.verbose);

    let result = if cfg.list {
        print_readers().map_err(|e| format!("listing readers: {e}"))
    } else if cfg.atr {
        print_atr(cfg.reader).map_err(|e| format!("reading ATR: {e}"))
    } else {
        run(cfg)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}

fn print_readers() -> Result<(), String> {
    let readers = list_readers().map_err(|e| e.to_string())?;
    if readers.is_empty() {
        println!("No readers found.");
    }
    for (i, name) in readers.iter().enumerate() {
        println!("{i} | {name}");
    }
    Ok(())
}

/// Connect to the reader with the given index.
fn connect(index: usize) -> Result<PcscCard, String> {
    let readers = list_readers().map_err(|e| format!("listing readers: {e}"))?;
    if readers.is_empty() {
        return Err("no reader found".into());
    }
    let name = readers
        .get(index)
        .ok_or_else(|| format!("only {} readers found", readers.len()))?;
    log::info!("using reader {index}: {name}");
    PcscCard::connect(name).map_err(|e| format!("connecting reader {name}: {e}"))
}

fn print_atr(index: usize) -> Result<(), String> {
    let card = connect(index)?;
    let atr = card.atr().map_err(|e| e.to_string())?;
    println!(
        "{}",
        atr.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    Ok(())
}

/// Fail early when an output path exists but cannot be inspected (stat check).
fn check_path(path: &Option<String>) -> Result<(), String> {
    if let Some(p) = path {
        if let Err(e) = std::fs::metadata(p) {
            if e.kind() != ErrorKind::NotFound {
                return Err(format!("opening file {p}: {e}"));
            }
        }
    }
    Ok(())
}

fn write_file(path: &str, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts
        .open(Path::new(path))
        .map_err(|e| format!("writing file {path}: {e}"))?;
    f.write_all(data)
        .map_err(|e| format!("writing file {path}: {e}"))?;
    log::info!("saved {path}");
    Ok(())
}

fn run(mut cfg: Config) -> Result<(), String> {
    if cfg.pdf_path.is_none() && cfg.json_path.is_none() && cfg.excel_path.is_none() {
        log::info!("no output file path detected, using default value");
        cfg.json_path = Some("out.json".into());
    }

    check_path(&cfg.pdf_path)?;
    check_path(&cfg.json_path)?;
    check_path(&cfg.excel_path)?;

    let card = connect(cfg.reader)?;
    let mut doc = read_document(&card).map_err(|e| format!("reading card: {e}"))?;
    drop(card);

    if let Document::Medical(m) = &mut doc {
        if cfg.rfzo_valid_until {
            celik_core::rfzo::update_valid_until(m)
                .map_err(|e| format!("updating `ValidUntil` date: {e}"))?;
        }
    }

    if let Some(p) = &cfg.pdf_path {
        let pdf = celik_export::to_pdf(
            &doc,
            PdfOptions {
                cyrillic_labels: cfg.cyrillic_labels,
            },
        )
        .map_err(|e| format!("generating pdf: {e}"))?;
        write_file(p, &pdf)?;
    }

    if let Some(p) = &cfg.json_path {
        let json = celik_export::to_json(&doc).map_err(|e| format!("generating json: {e}"))?;
        write_file(p, &json)?;
    }

    if let Some(p) = &cfg.excel_path {
        let xlsx = celik_export::to_xlsx(&doc).map_err(|e| format!("generating excel: {e}"))?;
        write_file(p, &xlsx)?;
    }

    Ok(())
}
