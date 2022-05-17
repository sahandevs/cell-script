use crate::{ast_interpreter, parser::parse, scanner::scan};
use anyhow::bail;
use clap::Parser;
use itertools::Itertools;
use rayon::prelude::*;
use serde_json;
use std::{collections::HashMap, fmt::Display, path::PathBuf, str::FromStr};

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
    let mut param_names = Vec::new();
    let mut params_values = Vec::new();
    for param in &args.param {
        if let Some((name, values_str)) = param.split_once('=') {
            let mut values = vec![];
            for value in values_str.split(",") {
                let value: f64 = value.parse()?;
                values.push(value);
            }
            params_values.push(values);
            param_names.push(name.to_string());
        } else {
            bail!("invalid param. usage --param \"name=1\"")
        }
    }

    let permutations: Vec<_> = params_values
        .into_iter()
        .multi_cartesian_product()
        .par_bridge()
        .collect();
    let param_len = param_names.len();
    let outputs: Vec<_> = permutations
        .into_iter()
        .par_bridge()
        .flat_map(|permutation| {
            let mut input = HashMap::with_capacity(param_len);
            for (name, value) in param_names.iter().zip(permutation.iter()) {
                input.insert(name.to_string(), *value);
            }
            let result = ast_interpreter::run(&ast, &args.query, &input).ok()?;
            let output = Output {
                input,
                output: result,
            };
            Some(output)
        })
        .collect();

    match args.format {
        OutputFormat::Text => {
            for output in outputs.into_iter() {
                println!(
                    "{:?}({:?}) = {}",
                    args.code_path, output.input, output.output
                );
            }
        }
        OutputFormat::Json => {
            let output = serde_json::to_string_pretty(&outputs)?;
            println!("{}", output);
        }
    }
    Ok(())
}
