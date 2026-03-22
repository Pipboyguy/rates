use std::fs::{create_dir_all, read_to_string, write};

use chrono::prelude::*;
use clap::{CommandFactory, Parser, error::ErrorKind};
use colored::Colorize;
use directories::ProjectDirs;

fn get_rate(from: &str, to: &str, fiat_list: &str) -> f64 {
    let json: serde_json::Value =
        serde_json::from_str(fiat_list).expect("The result doesn't seem to be JSON");
    let rates = &json["rates"];

    let lookup = |code: &str| -> f64 {
        if code == "USD" {
            1.0
        } else {
            rates[code]
                .as_f64()
                .unwrap_or_else(|| panic!("Currency \"{}\" is not available.", code))
        }
    };

    lookup(to) / lookup(from)
}

fn fetch_data(url: &str) -> Result<String, reqwest::Error> {
    let body = reqwest::blocking::get(url)?.text()?;
    Ok(body)
}

fn init_currency_data(force_cache_update: bool) -> String {
    let proj_dirs = ProjectDirs::from("rs", "Lunush", "Rates").unwrap();
    let cache_dir = proj_dirs.cache_dir().to_str().unwrap().to_owned();
    let fiat_list_path = format!("{}/fiat_list.json", cache_dir);
    let last_update_path = format!("{}/last_update", cache_dir);

    if let Err(why) = create_dir_all(&cache_dir) {
        panic!("Unable to create {} folder:\n\n{}", cache_dir, why);
    };

    let now = Utc::now().timestamp();
    let needs_update = force_cache_update
        || match read_to_string(&last_update_path) {
            Ok(time) => {
                let last_update_time = time.parse::<i64>().unwrap();
                last_update_time + 3600 * 3 < now
            }
            Err(_) => true,
        };

    if needs_update {
        let fiat_list = fetch_data("https://open.er-api.com/v6/latest/USD").unwrap();
        cache_data(&fiat_list_path, &fiat_list);
        cache_data(&last_update_path, &now.to_string());
        fiat_list
    } else {
        read_cache(&fiat_list_path)
    }
}

fn read_cache(path: &str) -> String {
    match read_to_string(path) {
        Ok(str) => str,
        Err(why) => panic!("An error occured while reading the cache: {}", why),
    }
}

fn cache_data(path: &str, data: &str) {
    match write(path, data) {
        Ok(_) => (),
        Err(why) => panic!("An error occured during caching: {}", why),
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "rates",
    about = "Currency exchange rates in your terminal",
    after_help = "EXAMPLES:\n  rates USD ZAR          1 USD in ZAR\n  rates 100 USD ZAR      100 USD in ZAR\n  rates EUR to GBP       1 EUR in GBP\n  rates -s USD ZAR       number only"
)]
struct Args {
    /// Amount (e.g. 100) or source currency (e.g. USD)
    #[arg(value_name = "AMOUNT|FROM")]
    arg1: String,

    /// Source currency if amount given, or target currency
    #[arg(value_name = "FROM|TO")]
    arg2: Option<String>,

    /// Target currency or the word "to"
    #[arg(value_name = "TO")]
    arg3: Option<String>,

    /// Target currency (when using "to" syntax)
    #[arg(value_name = "TO")]
    arg4: Option<String>,

    /// Show only the result
    #[arg(short = 's', long = "short")]
    short: bool,

    /// Trim the digits after decimal point, if any
    #[arg(short = 't', long = "trim")]
    trim: bool,

    /// Do not format the result
    #[arg(short = 'F', long = "noformatting")]
    no_formatting: bool,

    /// Forcefully update currency data
    #[arg(short = 'f', long = "force")]
    force_cache_update: bool,
}

fn parse_args(args: &Args) -> (String, String, f64) {
    let arg1 = &args.arg1;
    let arg2 = args.arg2.clone();
    let arg3 = args.arg3.clone();
    let arg4 = args.arg4.clone();

    if let Ok(amount) = arg1.parse::<f64>() {
        let from = arg2
            .unwrap_or_else(|| {
                Args::command()
                    .error(
                        ErrorKind::MissingRequiredArgument,
                        "currency code required after amount (e.g. rates 100 USD ZAR)",
                    )
                    .exit()
            })
            .to_uppercase();
        let to = match arg3 {
            Some(arg) if arg == "to" => arg4.map(|a| a.to_uppercase()).unwrap_or("EUR".into()),
            Some(arg) => arg.to_uppercase(),
            None => "EUR".into(),
        };
        (from, to, amount)
    } else {
        let from = arg1.to_uppercase();
        let to = match arg2 {
            Some(arg) if arg == "to" => arg3.map(|a| a.to_uppercase()).unwrap_or("EUR".into()),
            Some(arg) => arg.to_uppercase(),
            None => "EUR".into(),
        };
        (from, to, 1.0)
    }
}

fn main() {
    let args = Args::parse();
    let (from, to, amount) = parse_args(&args);
    let fiat_list = init_currency_data(args.force_cache_update);

    let mut to_val = amount * get_rate(&from, &to, &fiat_list);

    let digits = to_val.to_string().chars().collect::<Vec<_>>();

    if args.trim {
        to_val = to_val.floor();
    } else if !args.no_formatting && digits.len() > 3 {
        let mut decimal_length = 3;
        let decimal_point_index = digits.iter().position(|x| *x == '.').unwrap_or(0);

        if to_val < 1.0 && decimal_point_index != 0 {
            for digit in digits.iter().skip(decimal_point_index + 1) {
                if *digit != '0' {
                    break;
                }
                decimal_length += 1;
            }
        }

        let len = (decimal_point_index + decimal_length).min(digits.len());
        to_val = digits[0..len]
            .iter()
            .collect::<String>()
            .parse::<f64>()
            .unwrap();
    }

    if args.short {
        println!("{}", to_val);
    } else {
        println!(
            "{} {} {} {} {}",
            amount.to_string().bold(),
            from.cyan(),
            "=".dimmed(),
            to_val.to_string().bold().green(),
            to.cyan()
        );
    };
}
