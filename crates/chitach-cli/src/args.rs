//! Command-line flags. Port of bas-celik (MIT, (c) Nikola Ubavić) `internal/flags.go`.
//!
//! Accepts both GNU-style (`--pdf out.pdf`, `--pdf=out.pdf`) and Go `flag`
//! style spellings (`-pdf out.pdf`, `-pdf=out.pdf`, `-verbose=false`), plus the
//! original `-rfzoValidUntil` spelling.

use std::fmt;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Config {
    pub atr: bool,
    pub cyrillic_labels: bool,
    pub excel_path: Option<String>,
    pub json_path: Option<String>,
    pub pdf_path: Option<String>,
    pub list: bool,
    pub reader: usize,
    pub rfzo_valid_until: bool,
    pub verbose: bool,
    pub version: bool,
    pub help: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub const USAGE: &str = "\
Usage: chitach-cli [OPTIONS]

Reads a Serbian eID, medical insurance card or vehicle registration card and
exports it. Without any export option the document is saved to out.json.

Options:
  --atr                  Print the ATR from the card and exit
  --cyrillic-labels      Print ID PDF with Cyrillic labels. Ignored for other cards
  --excel PATH           Set Excel (XLSX) export path
  --json PATH            Set JSON export path
  --pdf PATH             Set PDF export path
  --list                 List connected readers and exit
  --reader INDEX         Set reader index (see --list), default 0
  --rfzo-valid-until     Get the \"valid until\" date of a medical card from the RFZO
                         service. Ignored for other cards
  --verbose              Provide additional details in the terminal
  --version              Display version information and exit
  -h, --help             Show this help and exit

Single-dash spellings (-pdf PATH, -json=PATH, -rfzoValidUntil) are accepted too.
";

enum Kind {
    Bool(fn(&mut Config, bool)),
    Str(fn(&mut Config, String)),
    Index,
}

fn lookup(name: &str) -> Option<Kind> {
    Some(match name {
        "atr" => Kind::Bool(|c, v| c.atr = v),
        "cyrillic-labels" | "cyrillicLabels" => Kind::Bool(|c, v| c.cyrillic_labels = v),
        "list" => Kind::Bool(|c, v| c.list = v),
        "rfzo-valid-until" | "rfzoValidUntil" => Kind::Bool(|c, v| c.rfzo_valid_until = v),
        "verbose" => Kind::Bool(|c, v| c.verbose = v),
        "version" => Kind::Bool(|c, v| c.version = v),
        "help" | "h" => Kind::Bool(|c, v| c.help = v),
        "excel" => Kind::Str(|c, v| c.excel_path = Some(v)),
        "json" => Kind::Str(|c, v| c.json_path = Some(v)),
        "pdf" => Kind::Str(|c, v| c.pdf_path = Some(v)),
        "reader" => Kind::Index,
        _ => return None,
    })
}

fn parse_bool(s: &str) -> Option<bool> {
    // Same values Go's strconv.ParseBool accepts.
    match s {
        "1" | "t" | "T" | "true" | "TRUE" | "True" => Some(true),
        "0" | "f" | "F" | "false" | "FALSE" | "False" => Some(false),
        _ => None,
    }
}

/// Parse arguments (without the program name).
pub fn parse<I, S>(args: I) -> Result<Config, UsageError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut cfg = Config::default();
    let mut it = args.into_iter().map(Into::into);
    while let Some(arg) = it.next() {
        let Some(stripped) = arg.strip_prefix("--").or_else(|| arg.strip_prefix('-')) else {
            return Err(UsageError(format!("unexpected argument: {arg}")));
        };
        if stripped.is_empty() || stripped.starts_with('-') {
            return Err(UsageError(format!("bad flag syntax: {arg}")));
        }
        let (name, inline) = match stripped.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (stripped, None),
        };
        let kind = lookup(name)
            .ok_or_else(|| UsageError(format!("flag provided but not defined: -{name}")))?;
        match kind {
            Kind::Bool(set) => {
                let value = match inline {
                    Some(v) => parse_bool(&v).ok_or_else(|| {
                        UsageError(format!("invalid boolean value {v:?} for -{name}"))
                    })?,
                    None => true,
                };
                set(&mut cfg, value);
            }
            Kind::Str(set) => {
                let value = match inline {
                    Some(v) => v,
                    None => it
                        .next()
                        .ok_or_else(|| UsageError(format!("flag needs an argument: -{name}")))?,
                };
                set(&mut cfg, value);
            }
            Kind::Index => {
                let value = match inline {
                    Some(v) => v,
                    None => it
                        .next()
                        .ok_or_else(|| UsageError(format!("flag needs an argument: -{name}")))?,
                };
                cfg.reader = value.parse().map_err(|_| {
                    UsageError(format!(
                        "invalid value {value:?} for -{name}: expected a reader index"
                    ))
                })?;
            }
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        assert_eq!(parse(Vec::<String>::new()).unwrap(), Config::default());
    }

    #[test]
    fn gnu_and_go_spellings() {
        let c = parse([
            "--pdf",
            "a.pdf",
            "-json=b.json",
            "--excel=c.xlsx",
            "-reader",
            "2",
            "-verbose",
            "--cyrillic-labels",
        ])
        .unwrap();
        assert_eq!(c.pdf_path.as_deref(), Some("a.pdf"));
        assert_eq!(c.json_path.as_deref(), Some("b.json"));
        assert_eq!(c.excel_path.as_deref(), Some("c.xlsx"));
        assert_eq!(c.reader, 2);
        assert!(c.verbose && c.cyrillic_labels);
        assert!(!c.atr && !c.list);
    }

    #[test]
    fn rfzo_spellings() {
        assert!(parse(["-rfzoValidUntil"]).unwrap().rfzo_valid_until);
        assert!(parse(["--rfzo-valid-until"]).unwrap().rfzo_valid_until);
        assert!(
            !parse(["--rfzo-valid-until=false"])
                .unwrap()
                .rfzo_valid_until
        );
    }

    #[test]
    fn help_version() {
        assert!(parse(["-h"]).unwrap().help);
        assert!(parse(["--help"]).unwrap().help);
        assert!(parse(["-version"]).unwrap().version);
    }

    #[test]
    fn errors() {
        assert!(parse(["--nope"]).is_err());
        assert!(parse(["--pdf"]).is_err());
        assert!(parse(["--reader", "x"]).is_err());
        assert!(parse(["--reader=-1"]).is_err());
        assert!(parse(["--verbose=maybe"]).is_err());
        assert!(parse(["positional"]).is_err());
        assert!(parse(["---pdf", "x"]).is_err());
    }
}
