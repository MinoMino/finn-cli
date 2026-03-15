# finn-cli

A Rust CLI for [FINN.no](https://www.finn.no/) Torget using FINN's unofficial web endpoints and page hydration data.

## What it supports

- Search Torget listings
- Human-readable output
- JSON output to stdout
- JSON output to a file
- Item/ad detail lookup by FINN id or URL
- Sorts, common filters, and raw passthrough query params
- Pagination helpers for fetching multiple pages
- Category lookup by the actual words used on the site
- Category aliases and typo-tolerant matching
- Interactive category picker
- Category listing via `categories`

## Install

Build a release binary:

```bash
cargo build --release
./target/release/finn-cli --help
```

Or install it locally:

```bash
cargo install --path .
```

## Usage

### Search

Human-readable:

```bash
finn-cli search rtx 4080 --category "Elektronikk og hvitevarer"
```

JSON to stdout:

```bash
finn-cli search rtx 4080 --category "Elektronikk og hvitevarer" --json
```

JSON to file:

```bash
finn-cli search rtx 4080 --category "Elektronikk og hvitevarer" --output results.json
```

Fetch all pages:

```bash
finn-cli search rtx 4080 --category "Elektronikk og hvitevarer" --all-pages --json
```

Sort and filter:

```bash
finn-cli search rtx 4080 \
  --category "Elektronikk og hvitevarer > Data > Datakomponenter" \
  --sort price-asc \
  --price-from 10000 \
  --price-to 20000 \
  --trade-type for-sale \
  --dealer-segment private \
  --condition like-new \
  --shipping
```

Aliases and typo-tolerant matching:

```bash
finn-cli search rtx 4080 --category electronics
finn-cli search rtx 4080 --category datakomponnter
```

Interactive picker:

```bash
finn-cli search rtx 4080 --pick-category
finn-cli search rtx 4080 --pick-category --category data
```

Passthrough query params:

```bash
finn-cli search rtx 4080 --param location=0.20061 --param published=1
```

### Categories

List all categories:

```bash
finn-cli categories
```

Filter categories by text, alias, or typo:

```bash
finn-cli categories data
finn-cli categories electronics
finn-cli categories datakomponnter
```

Interactive picker:

```bash
finn-cli categories --interactive
finn-cli categories data --interactive
```

JSON output:

```bash
finn-cli categories --json
```

### Item detail

By id:

```bash
finn-cli item 451260160
```

By URL:

```bash
finn-cli item https://www.finn.no/recommerce/forsale/item/451260160 --json
```

## Notes

- This project uses FINN's web-facing endpoints and HTML hydration data, not a public business API.
- It is intended for personal CLI use and may break if FINN changes its frontend.
- The main search endpoint used is:
  - `/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON`
