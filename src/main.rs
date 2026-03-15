use clap::{Args, Parser, Subcommand};
use finn_cli::{
    ConditionFilter, DealerSegmentFilter, FinnClient, ForRentFilter, SearchParams, SearchSort,
    TradeTypeFilter, render_categories, render_item_detail, render_search_result,
    suggest_categories,
};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "Search FINN Torget from the command line")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Search(SearchArgs),
    Item(ItemArgs),
    Categories(CategoriesArgs),
}

#[derive(Debug, Args, Clone)]
struct OutputArgs {
    #[arg(long, help = "Emit JSON to stdout instead of human-readable output")]
    json: bool,
    #[arg(long, short, help = "Write JSON output to this file")]
    output: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct SearchArgs {
    #[arg(required = true)]
    query: Vec<String>,
    #[arg(
        long,
        help = "FINN category id or category text, e.g. 'Elektronikk og hvitevarer' or 'Elektronikk og hvitevarer > Data > Datakomponenter'"
    )]
    category: Option<String>,
    #[arg(long, help = "Interactively pick a category before searching")]
    pick_category: bool,
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    page: u32,
    #[arg(long, value_enum)]
    sort: Option<SearchSort>,
    #[arg(long)]
    price_from: Option<u64>,
    #[arg(long)]
    price_to: Option<u64>,
    #[arg(long, help = "Only show items that support Fiks ferdig")]
    shipping: bool,
    #[arg(long = "location", help = "FINN location id, e.g. 0.20061 for Oslo")]
    locations: Vec<String>,
    #[arg(long, value_enum)]
    for_rent: Option<ForRentFilter>,
    #[arg(long, value_enum)]
    trade_type: Option<TradeTypeFilter>,
    #[arg(long, value_enum)]
    dealer_segment: Option<DealerSegmentFilter>,
    #[arg(long = "condition", value_enum)]
    conditions: Vec<ConditionFilter>,
    #[arg(long, help = "Only show ads published today")]
    published_today: bool,
    #[arg(long, help = "Fetch all pages starting from --page")]
    all_pages: bool,
    #[arg(
        long,
        requires = "all_pages",
        value_parser = clap::value_parser!(u32).range(1..),
        help = "Maximum number of pages to fetch when --all-pages is set"
    )]
    max_pages: Option<u32>,
    #[arg(
        long = "param",
        value_parser = parse_key_value,
        help = "Raw query parameter passthrough in key=value form"
    )]
    params: Vec<(String, String)>,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct ItemArgs {
    item: String,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Debug, Args)]
struct CategoriesArgs {
    #[arg(help = "Optional text filter for category names or paths")]
    filter: Vec<String>,
    #[arg(long, help = "Interactively pick a category")]
    interactive: bool,
    #[command(flatten)]
    output: OutputArgs,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let base_url =
        std::env::var("FINN_BASE_URL").unwrap_or_else(|_| "https://www.finn.no".to_string());
    let client = FinnClient::with_base_url(base_url)?;

    match cli.command {
        Commands::Search(args) => {
            if let (Some(min), Some(max)) = (args.price_from, args.price_to) {
                if min > max {
                    return Err("--price-from cannot be greater than --price-to".into());
                }
            }

            let category = if args.pick_category {
                Some(
                    pick_category(&client, args.category.as_deref())
                        .await?
                        .path_string(),
                )
            } else {
                args.category.clone()
            };

            let mut params = SearchParams::new(args.query.join(" ")).with_page(args.page);
            if let Some(category) = category {
                params = params.with_category(category);
            }
            if let Some(sort) = args.sort {
                params = params.with_sort(sort);
            }
            if let Some(price_from) = args.price_from {
                params = params.with_price_from(price_from);
            }
            if let Some(price_to) = args.price_to {
                params = params.with_price_to(price_to);
            }
            if args.shipping {
                params = params.with_shipping(true);
            }
            if let Some(for_rent) = args.for_rent {
                params = params.with_for_rent(for_rent);
            }
            if let Some(trade_type) = args.trade_type {
                params = params.with_trade_type(trade_type);
            }
            if let Some(dealer_segment) = args.dealer_segment {
                params = params.with_dealer_segment(dealer_segment);
            }
            if args.published_today {
                params = params.with_published_today(true);
            }
            for location in args.locations {
                params = params.with_location(location);
            }
            for condition in args.conditions {
                params = params.with_condition(condition);
            }
            for (key, value) in args.params {
                params = params.with_raw_param(key, value);
            }

            let result = if args.all_pages {
                client.search_all(&params, args.max_pages).await?
            } else {
                client.search(&params).await?
            };

            emit_output(
                result.to_json_pretty()?,
                render_search_result(&result),
                &args.output,
            )?;
        }
        Commands::Item(args) => {
            let item = client.get_item(&args.item).await?;
            emit_output(
                item.to_json_pretty()?,
                render_item_detail(&item),
                &args.output,
            )?;
        }
        Commands::Categories(args) => {
            let categories = client.categories().await?;

            if args.interactive {
                let picked = pick_from_categories(&categories, args.filter.join(" ").trim())?;
                emit_output(
                    serde_json::to_string_pretty(&picked)?,
                    render_categories(&[picked]),
                    &args.output,
                )?;
            } else {
                let filtered = if args.filter.is_empty() {
                    categories
                } else {
                    suggest_categories(&categories, &args.filter.join(" "), 50)
                };

                emit_output(
                    serde_json::to_string_pretty(&filtered)?,
                    render_categories(&filtered),
                    &args.output,
                )?;
            }
        }
    }

    Ok(())
}

fn emit_output(
    json: String,
    human: String,
    output: &OutputArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = &output.output {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json)?;
    } else if output.json {
        println!("{json}");
    } else {
        println!("{human}");
    }

    Ok(())
}

fn parse_key_value(input: &str) -> Result<(String, String), String> {
    let (key, value) = input
        .split_once('=')
        .ok_or_else(|| "expected key=value".to_string())?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err("expected non-empty key=value".to_string());
    }
    Ok((key.to_string(), value.to_string()))
}

async fn pick_category(
    client: &FinnClient,
    initial_filter: Option<&str>,
) -> Result<finn_cli::CategoryEntry, Box<dyn std::error::Error>> {
    let categories = client.categories().await?;
    pick_from_categories(&categories, initial_filter.unwrap_or_default())
}

fn pick_from_categories(
    categories: &[finn_cli::CategoryEntry],
    initial_filter: &str,
) -> Result<finn_cli::CategoryEntry, Box<dyn std::error::Error>> {
    let mut filter = initial_filter.trim().to_string();

    loop {
        let suggestions = if filter.is_empty() {
            categories.iter().take(15).cloned().collect::<Vec<_>>()
        } else {
            suggest_categories(categories, &filter, 15)
        };

        if suggestions.is_empty() {
            eprintln!("No categories matched '{filter}'. Enter a new filter.");
            filter = prompt("Category filter: ")?;
            continue;
        }

        eprintln!("Select a category:");
        for (index, category) in suggestions.iter().enumerate() {
            eprintln!(
                "  {}. {} ({}={})",
                index + 1,
                category.path_string(),
                category.query_param,
                category.value
            );
        }
        if filter.is_empty() {
            eprintln!("Type a number, or enter filter text to narrow the list.");
        } else {
            eprintln!("Type a number, press Enter for #1, or enter new filter text.");
        }

        let answer = prompt("Choice: ")?;
        let trimmed = answer.trim();

        if trimmed.is_empty() && !filter.is_empty() {
            return Ok(suggestions[0].clone());
        }

        if let Ok(index) = trimmed.parse::<usize>() {
            if (1..=suggestions.len()).contains(&index) {
                return Ok(suggestions[index - 1].clone());
            }
            eprintln!("Invalid selection: {index}");
            continue;
        }

        filter = trimmed.to_string();
    }
}

fn prompt(label: &str) -> Result<String, Box<dyn std::error::Error>> {
    eprint!("{label}");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end().to_string())
}
