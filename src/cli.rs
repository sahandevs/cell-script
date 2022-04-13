use anyhow::bail;
use clap::Parser;
use serde_json;
use std::{collections::HashMap, fmt::Display, path::PathBuf, str::FromStr};

use crate::{ast_interpreter, parser::parse, scanner::scan};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    code_path: PathBuf,

    #[clap(short, long, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    #[clap(short, long)]
    query: String,

    #[clap(short, long)]
    param: Vec<String>,
}

#[derive(Debug)]
pub enum OutputFormat {
    Text,
    Json,
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl FromStr for OutputFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => bail!("unrecognized output format `{}`", s),
        }
    }
}

#[derive(Debug, serde::Serialize)]
struct Output {
    input: HashMap<String, f64>,
    output: f64,
}

pub fn run() -> Result<(), anyhow::Error> {
    let args = Args::parse();

    // parse code and build AST
    let ast = {
        let content = std::fs::read_to_string(&args.code_path)?;
        let tokens = scan(&content)?;
        parse(tokens)?
    };

    // build params
    let mut params = HashMap::new();
    for param in &args.param {
        if let Some((name, values_str)) = param.split_once('=') {
            let value: f64 = values_str.trim().parse()?;
            params.insert(name.to_owned(), value);
        } else {
            bail!("invalid param. usage --param \"name=1\"")
        }
    }

    let result = ast_interpreter::run(&ast, &args.query, &params)?;

    match args.format {
        OutputFormat::Text => {
            println!("{:?} {:?} = {}", args.param, args.code_path, result);
        }
        OutputFormat::Json => {
            let output = Output {
                input: params,
                output: result,
            };
            let output = serde_json::to_string_pretty(&output)?;
            println!("{}", output);
        }
    }
    Ok(())
}
