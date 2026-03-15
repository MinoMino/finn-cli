use crate::model::{
    CategoryEntry, Coordinates, ItemDetail, Price, SearchItem, SearchParams, SearchResult,
};
use chrono::{TimeZone, Utc};
use regex::Regex;
use reqwest::header::ACCEPT;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use strsim::{jaro_winkler, normalized_levenshtein};

const DEFAULT_BASE_URL: &str = "https://www.finn.no";
const SEARCH_PATH: &str = "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON";
const ITEM_PATH_PREFIX: &str = "/recommerce/forsale/item/";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
const DEFAULT_CATEGORY_SUGGESTION_LIMIT: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum FinnError {
    #[error("search query must not be empty")]
    EmptyQuery,
    #[error("request to FINN failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("failed to decode FINN data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid ad id in response: {0}")]
    InvalidAdId(String),
    #[error("invalid item identifier: {0}")]
    InvalidItemIdentifier(String),
    #[error("item page did not contain hydration data")]
    MissingHydrationData,
    #[error(
        "unknown category: {input}. {suggestions} Try `finn-cli categories` or `finn-cli search ... --pick-category`."
    )]
    UnknownCategory { input: String, suggestions: String },
    #[error(
        "ambiguous category: {input}. Possible matches: {matches}. Use a fuller path or `--pick-category`."
    )]
    AmbiguousCategory { input: String, matches: String },
}

#[derive(Debug, Clone)]
pub struct FinnClient {
    http: reqwest::Client,
    base_url: String,
}

impl FinnClient {
    pub fn new() -> Result<Self, FinnError> {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, FinnError> {
        let http = reqwest::Client::builder().user_agent(USER_AGENT).build()?;

        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    pub async fn categories(&self) -> Result<Vec<CategoryEntry>, FinnError> {
        let url = format!("{}{}", self.base_url, SEARCH_PATH);
        let response = self
            .http
            .get(url)
            .header(ACCEPT, "application/json")
            .send()
            .await?
            .error_for_status()?;

        let api: ApiCategoriesResponse = response.json().await?;
        let Some(filter) = api
            .filters
            .into_iter()
            .find(|filter| filter.name == "category")
        else {
            return Ok(Vec::new());
        };

        let mut categories = Vec::new();
        flatten_category_entries(&filter.filter_items, &mut Vec::new(), &mut categories);
        categories.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(categories)
    }

    pub async fn categories_matching(
        &self,
        input: &str,
        limit: usize,
    ) -> Result<Vec<CategoryEntry>, FinnError> {
        let categories = self.categories().await?;
        Ok(suggest_categories(&categories, input, limit))
    }

    pub async fn search(&self, params: &SearchParams) -> Result<SearchResult, FinnError> {
        if params.query.trim().is_empty() {
            return Err(FinnError::EmptyQuery);
        }

        let resolved_category = self
            .resolve_category_input(params.category.as_deref())
            .await?;
        self.search_page_with_category(params, params.page.max(1), resolved_category.as_ref())
            .await
    }

    pub async fn search_all(
        &self,
        params: &SearchParams,
        max_pages: Option<u32>,
    ) -> Result<SearchResult, FinnError> {
        if params.query.trim().is_empty() {
            return Err(FinnError::EmptyQuery);
        }

        let resolved_category = self
            .resolve_category_input(params.category.as_deref())
            .await?;
        let mut combined = self
            .search_page_with_category(params, params.page.max(1), resolved_category.as_ref())
            .await?;
        let start_page = combined.current_page;
        let page_limit = max_pages.unwrap_or(u32::MAX).max(1);
        let target_last_page = start_page
            .saturating_add(page_limit.saturating_sub(1))
            .min(combined.last_page);

        for page in (start_page + 1)..=target_last_page {
            let next = self
                .search_page_with_category(params, page, resolved_category.as_ref())
                .await?;
            combined.items.extend(next.items);
        }

        combined.returned_items = combined.items.len();
        combined.fetched_pages = target_last_page.saturating_sub(start_page) + 1;
        combined.fetched_from_page = start_page;
        combined.fetched_to_page = target_last_page;
        combined.is_end_of_paging = target_last_page >= combined.last_page;

        Ok(combined)
    }

    pub async fn get_item(&self, item: &str) -> Result<ItemDetail, FinnError> {
        let url = self.resolve_item_url(item)?;
        let html = self
            .http
            .get(&url)
            .header(ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let hydration = extract_item_hydration(&html)?;
        map_item_detail(hydration, &url)
    }

    async fn resolve_category_input(
        &self,
        input: Option<&str>,
    ) -> Result<Option<CategoryEntry>, FinnError> {
        let Some(input) = input.map(str::trim).filter(|input| !input.is_empty()) else {
            return Ok(None);
        };

        if let Some(category) = parse_category_id(input) {
            return Ok(Some(category));
        }

        let categories = self.categories().await?;
        resolve_category_name(input, &categories).map(Some)
    }

    async fn search_page_with_category(
        &self,
        params: &SearchParams,
        page: u32,
        category: Option<&CategoryEntry>,
    ) -> Result<SearchResult, FinnError> {
        let url = format!("{}{}", self.base_url, SEARCH_PATH);
        let query = build_search_query(params, page.max(1), category);

        let response = self
            .http
            .get(url)
            .header(ACCEPT, "application/json")
            .query(&query)
            .send()
            .await?
            .error_for_status()?;

        let api: ApiSearchResponse = response.json().await?;
        map_search_response(params, api)
    }

    fn resolve_item_url(&self, item: &str) -> Result<String, FinnError> {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            return Err(FinnError::InvalidItemIdentifier(item.to_string()));
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return Ok(trimmed.to_string());
        }

        let id = extract_item_id(trimmed)
            .ok_or_else(|| FinnError::InvalidItemIdentifier(trimmed.to_string()))?;

        Ok(format!("{}{}{}", self.base_url, ITEM_PATH_PREFIX, id))
    }
}

pub fn suggest_categories(
    categories: &[CategoryEntry],
    input: &str,
    limit: usize,
) -> Vec<CategoryEntry> {
    let limit = limit.max(1);
    let normalized_input = normalize_category(input);

    if normalized_input.is_empty() {
        return categories.iter().take(limit).cloned().collect();
    }

    let ranked = rank_categories(categories, &normalized_input);
    let best_score = ranked.first().map(|entry| entry.score).unwrap_or(0.0);
    let threshold = (best_score - 0.20).max(0.65);

    ranked
        .into_iter()
        .filter(|scored| scored.score >= threshold)
        .take(limit)
        .map(|scored| scored.category)
        .collect()
}

fn build_search_query(
    params: &SearchParams,
    page: u32,
    category: Option<&CategoryEntry>,
) -> Vec<(String, String)> {
    let mut query = vec![
        ("q".to_string(), params.query.clone()),
        ("page".to_string(), page.max(1).to_string()),
    ];

    if let Some(category) = category {
        query.push((category.query_param.clone(), category.value.clone()));
    }

    if let Some(sort) = params.sort {
        query.push(("sort".to_string(), sort.as_api_value().to_string()));
    }
    if let Some(price_from) = params.price_from {
        query.push(("price_from".to_string(), price_from.to_string()));
    }
    if let Some(price_to) = params.price_to {
        query.push(("price_to".to_string(), price_to.to_string()));
    }
    if params.shipping {
        query.push(("shipping_types".to_string(), "0".to_string()));
    }
    for location in &params.locations {
        if !location.trim().is_empty() {
            query.push(("location".to_string(), location.clone()));
        }
    }
    if let Some(for_rent) = params.for_rent {
        query.push(("for_rent".to_string(), for_rent.as_api_value().to_string()));
    }
    if let Some(trade_type) = params.trade_type {
        query.push((
            "trade_type".to_string(),
            trade_type.as_api_value().to_string(),
        ));
    }
    if let Some(dealer_segment) = params.dealer_segment {
        query.push((
            "dealer_segment".to_string(),
            dealer_segment.as_api_value().to_string(),
        ));
    }
    for condition in &params.conditions {
        query.push((
            "condition".to_string(),
            condition.as_api_value().to_string(),
        ));
    }
    if params.published_today {
        query.push(("published".to_string(), "1".to_string()));
    }
    for (key, value) in &params.raw_params {
        if !key.trim().is_empty() && !value.trim().is_empty() {
            query.push((key.clone(), value.clone()));
        }
    }

    query
}

fn map_search_response(
    params: &SearchParams,
    api: ApiSearchResponse,
) -> Result<SearchResult, FinnError> {
    let metadata = api.metadata;
    let returned_items = api.docs.len();
    let items = api
        .docs
        .into_iter()
        .map(map_search_item)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SearchResult {
        query: params.query.clone(),
        category: params.category.clone(),
        title: metadata.title,
        marketplace: metadata.search_key_description,
        current_page: metadata.paging.current,
        last_page: metadata.paging.last,
        fetched_pages: 1,
        fetched_from_page: metadata.paging.current,
        fetched_to_page: metadata.paging.current,
        total_matches: metadata.result_size.match_count,
        returned_items,
        is_end_of_paging: metadata.is_end_of_paging,
        items,
    })
}

fn map_search_item(api: ApiItem) -> Result<SearchItem, FinnError> {
    let id = match api.ad_id {
        Some(id) => id,
        None => api
            .id
            .parse::<u64>()
            .map_err(|_| FinnError::InvalidAdId(api.id.clone()))?,
    };

    let primary_image_url = api.image.as_ref().map(|image| image.url.clone());
    let labels = api.labels.into_iter().map(|label| label.text).collect();
    let coordinates = api.coordinates.map(|coordinates| Coordinates {
        lat: coordinates.lat,
        lon: coordinates.lon,
    });
    let extras = extras_to_map(api.extras);
    let published_at = api.timestamp.and_then(|timestamp| {
        Utc.timestamp_millis_opt(timestamp)
            .single()
            .map(|datetime| datetime.to_rfc3339())
    });

    Ok(SearchItem {
        id,
        title: api.heading,
        location: api.location,
        price: api.price.map(|price| Price {
            amount: price.amount,
            currency_code: price.currency_code,
            unit: price.price_unit,
        }),
        trade_type: api.trade_type,
        url: api.canonical_url,
        primary_image_url,
        image_urls: api.image_urls,
        labels,
        flags: api.flags,
        brand: api.brand,
        coordinates,
        timestamp_ms: api.timestamp,
        published_at,
        extras,
    })
}

fn map_item_detail(
    hydration: ItemHydrationRoot,
    fallback_url: &str,
) -> Result<ItemDetail, FinnError> {
    let route = hydration.loader_data.item_recommerce;
    let item = route.item_data;
    let price_currency = route
        .json_ld
        .as_ref()
        .and_then(|json_ld| json_ld.offers.as_ref())
        .map(|offer| offer.price_currency.clone())
        .unwrap_or_else(|| "NOK".to_string());
    let id = item
        .meta
        .ad_id
        .parse::<u64>()
        .map_err(|_| FinnError::InvalidAdId(item.meta.ad_id.clone()))?;
    let url = route
        .meta
        .canonical
        .or_else(|| route.json_ld.and_then(|json_ld| json_ld.url))
        .unwrap_or_else(|| fallback_url.to_string());
    let location = item.location.as_ref().and_then(format_item_location);
    let coordinates = item.location.as_ref().and_then(|location| {
        location.position.as_ref().map(|position| Coordinates {
            lat: position.lat,
            lon: position.lng,
        })
    });
    let (postal_code, postal_name, country_code, country_name) = match item.location.as_ref() {
        Some(location) => (
            location.postal_code.clone(),
            location.postal_name.clone(),
            location.country_code.clone(),
            location.country_name.clone(),
        ),
        None => (None, None, None, None),
    };
    let category_path = item
        .category
        .as_ref()
        .map(collect_category_path)
        .unwrap_or_default();
    let image_urls = item.images.into_iter().map(|image| image.uri).collect();
    let extras = item
        .extras
        .into_iter()
        .filter_map(|extra| {
            extra
                .value
                .filter(|value| !value.trim().is_empty())
                .map(|value| (extra.id, value))
        })
        .collect::<BTreeMap<_, _>>();
    let transactable = route.transactable_data.unwrap_or_default();

    Ok(ItemDetail {
        id,
        title: item.title,
        url,
        price: item.price.map(|amount| Price {
            amount,
            currency_code: price_currency,
            unit: "kr".to_string(),
        }),
        trade_type: item.ad_view_type_label,
        description: item.description,
        location,
        postal_code,
        postal_name,
        country_code,
        country_name,
        coordinates,
        category_path,
        image_urls,
        extras,
        edited_at: item.meta.edited,
        is_transactable: transactable.transactable,
        buy_now: transactable.buy_now,
        eligible_for_shipping: transactable.eligible_for_shipping,
        seller_pays_shipping: transactable.seller_pays_shipping,
        is_webstore: item.is_webstore,
        anonymous_seller: item.anonymous,
        is_inactive: item.meta.is_inactive,
        is_disposed: item.disposed,
    })
}

fn extract_item_hydration(html: &str) -> Result<ItemHydrationRoot, FinnError> {
    let regex = Regex::new(
        r#"window\.__staticRouterHydrationData\s*=\s*JSON\.parse\(("(?:(?:\\.)|[^"])*")\);"#,
    )
    .expect("static hydration regex to compile");
    let captures = regex
        .captures(html)
        .ok_or(FinnError::MissingHydrationData)?;
    let encoded = captures
        .get(1)
        .map(|value| value.as_str())
        .ok_or(FinnError::MissingHydrationData)?;
    let decoded: String = serde_json::from_str(encoded)?;
    Ok(serde_json::from_str(&decoded)?)
}

fn parse_category_id(input: &str) -> Option<CategoryEntry> {
    let trimmed = input.trim();
    if !trimmed.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
        return None;
    }

    let query_param = if trimmed.starts_with("2.") {
        "product_category"
    } else if trimmed.starts_with("1.") {
        "sub_category"
    } else {
        "category"
    };

    Some(CategoryEntry {
        label: trimmed.to_string(),
        path: vec![trimmed.to_string()],
        query_param: query_param.to_string(),
        value: trimmed.to_string(),
    })
}

fn resolve_category_name(
    input: &str,
    categories: &[CategoryEntry],
) -> Result<CategoryEntry, FinnError> {
    let normalized_input = normalize_category(input);
    if normalized_input.is_empty() {
        return Err(FinnError::UnknownCategory {
            input: input.to_string(),
            suggestions: "No category text was provided.".to_string(),
        });
    }

    let exact_path_matches = categories
        .iter()
        .filter(|category| normalize_category(&category.path_string()) == normalized_input)
        .collect::<Vec<_>>();
    if let Some(category) = resolve_single_match(input, exact_path_matches)? {
        return Ok(category.clone());
    }

    let exact_label_matches = categories
        .iter()
        .filter(|category| normalize_category(&category.label) == normalized_input)
        .collect::<Vec<_>>();
    if let Some(category) = resolve_single_match(input, exact_label_matches)? {
        return Ok(category.clone());
    }

    let exact_alias_matches = categories
        .iter()
        .filter(|category| {
            category_search_strings(category)
                .iter()
                .any(|candidate| normalize_category(candidate) == normalized_input)
        })
        .collect::<Vec<_>>();
    if let Some(category) = resolve_single_match(input, exact_alias_matches)? {
        return Ok(category.clone());
    }

    let substring_matches = categories
        .iter()
        .filter(|category| {
            category_search_strings(category)
                .iter()
                .map(|candidate| normalize_category(candidate))
                .any(|candidate| candidate.contains(&normalized_input))
        })
        .collect::<Vec<_>>();
    if let Some(category) = resolve_single_match(input, substring_matches)? {
        return Ok(category.clone());
    }

    let ranked = rank_categories(categories, &normalized_input);
    if let Some(best) = ranked.first() {
        let next_score = ranked.get(1).map(|entry| entry.score).unwrap_or(0.0);
        if best.score >= 0.94 || (best.score >= 0.88 && (best.score - next_score) >= 0.05) {
            return Ok(best.category.clone());
        }
    }

    let suggestions = ranked
        .iter()
        .take(DEFAULT_CATEGORY_SUGGESTION_LIMIT)
        .map(|scored| scored.category.path_string())
        .collect::<Vec<_>>();

    if suggestions.is_empty() {
        return Err(FinnError::UnknownCategory {
            input: input.to_string(),
            suggestions: "No close matches were found.".to_string(),
        });
    }

    if ranked
        .first()
        .map(|scored| scored.score)
        .unwrap_or_default()
        >= 0.70
    {
        return Err(FinnError::AmbiguousCategory {
            input: input.to_string(),
            matches: suggestions.join(", "),
        });
    }

    Err(FinnError::UnknownCategory {
        input: input.to_string(),
        suggestions: format!("Did you mean: {}?", suggestions.join(", ")),
    })
}

fn resolve_single_match<'a>(
    input: &str,
    matches: Vec<&'a CategoryEntry>,
) -> Result<Option<&'a CategoryEntry>, FinnError> {
    match matches.as_slice() {
        [] => Ok(None),
        [single] => Ok(Some(*single)),
        many => {
            let examples = many
                .iter()
                .take(DEFAULT_CATEGORY_SUGGESTION_LIMIT)
                .map(|category| category.path_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(FinnError::AmbiguousCategory {
                input: input.to_string(),
                matches: examples,
            })
        }
    }
}

#[derive(Debug, Clone)]
struct ScoredCategory {
    category: CategoryEntry,
    score: f64,
}

fn rank_categories(categories: &[CategoryEntry], normalized_input: &str) -> Vec<ScoredCategory> {
    let mut ranked = categories
        .iter()
        .filter_map(|category| {
            score_category(category, normalized_input).map(|score| (category, score))
        })
        .filter(|(_, score)| *score >= 0.45)
        .map(|(category, score)| ScoredCategory {
            category: category.clone(),
            score,
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.category.path.len().cmp(&right.category.path.len()))
            .then_with(|| {
                left.category
                    .path_string()
                    .cmp(&right.category.path_string())
            })
    });
    ranked
}

fn score_category(category: &CategoryEntry, normalized_input: &str) -> Option<f64> {
    let mut best: f64 = 0.0;

    for candidate in normalized_search_strings(category) {
        if candidate == normalized_input {
            return Some(1.0);
        }

        if candidate.contains(normalized_input) || normalized_input.contains(&candidate) {
            let coverage = normalized_input.len().min(candidate.len()) as f64
                / normalized_input.len().max(candidate.len()) as f64;
            best = best.max(0.92 + coverage * 0.07);
        }

        best = best.max(jaro_winkler(&candidate, normalized_input));
        best = best.max(normalized_levenshtein(&candidate, normalized_input));
        best = best.max(token_overlap_score(&candidate, normalized_input));
    }

    if best > 0.0 { Some(best) } else { None }
}

fn normalized_search_strings(category: &CategoryEntry) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for candidate in category_search_strings(category) {
        let normalized = normalize_category(&candidate);
        if !normalized.is_empty() {
            unique.insert(normalized);
        }
    }
    unique.into_iter().collect()
}

fn category_search_strings(category: &CategoryEntry) -> Vec<String> {
    let mut strings = vec![category.label.clone(), category.path_string()];
    strings.extend(category_aliases(category));
    strings
}

fn category_aliases(category: &CategoryEntry) -> Vec<String> {
    let label = normalize_category(&category.label);
    let mut aliases = Vec::new();

    if label == "elektronikk og hvitevarer" {
        aliases.extend([
            "electronics".to_string(),
            "appliances".to_string(),
            "technology".to_string(),
            "tech".to_string(),
        ]);
    }

    match label.as_str() {
        "data" => aliases.extend([
            "computer".to_string(),
            "computers".to_string(),
            "pc".to_string(),
            "it".to_string(),
        ]),
        "datakomponenter" => aliases.extend([
            "components".to_string(),
            "computer components".to_string(),
            "pc parts".to_string(),
        ]),
        "baerbar pc" => aliases.extend([
            "laptop".to_string(),
            "notebook".to_string(),
            "portable computer".to_string(),
        ]),
        "stasjonaer pc" => aliases.extend(["desktop".to_string(), "desktop computer".to_string()]),
        "mobiltelefoner" => aliases.extend([
            "phone".to_string(),
            "phones".to_string(),
            "smartphone".to_string(),
            "mobile".to_string(),
        ]),
        "sport og friluftsliv" => aliases.extend([
            "sport".to_string(),
            "sports".to_string(),
            "outdoor".to_string(),
            "outdoors".to_string(),
        ]),
        _ => {}
    }

    aliases
}

fn token_overlap_score(candidate: &str, input: &str) -> f64 {
    let candidate_tokens = candidate.split_whitespace().collect::<BTreeSet<_>>();
    let input_tokens = input.split_whitespace().collect::<BTreeSet<_>>();

    if candidate_tokens.is_empty() || input_tokens.is_empty() {
        return 0.0;
    }

    let common = candidate_tokens.intersection(&input_tokens).count() as f64;
    if common == 0.0 {
        return 0.0;
    }

    let recall = common / input_tokens.len() as f64;
    let precision = common / candidate_tokens.len() as f64;
    (recall * 0.7) + (precision * 0.3)
}

fn normalize_category(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.trim().chars() {
        match ch {
            'Æ' | 'æ' => out.push_str("ae"),
            'Ø' | 'ø' => out.push('o'),
            'Å' | 'å' => out.push('a'),
            '&' => out.push_str(" og "),
            '>' | '/' | '-' | '_' | ',' | '.' | ':' | ';' | '(' | ')' => out.push(' '),
            ch if ch.is_alphanumeric() || ch.is_whitespace() => out.push(ch.to_ascii_lowercase()),
            _ => out.push(' '),
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_item_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }

    if let Some(start) = trimmed.find("/item/") {
        let digits = trimmed[start + 6..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if !digits.is_empty() {
            return Some(digits);
        }
    }

    None
}

fn format_item_location(location: &ApiItemLocation) -> Option<String> {
    match (&location.postal_code, &location.postal_name) {
        (Some(code), Some(name)) if !code.is_empty() && !name.is_empty() => {
            Some(format!("{code} {name}"))
        }
        (None, Some(name)) if !name.is_empty() => Some(name.clone()),
        (Some(code), None) if !code.is_empty() => Some(code.clone()),
        _ => location.country_name.clone(),
    }
}

fn collect_category_path(category: &ApiCategory) -> Vec<String> {
    let mut path = match category.parent.as_deref() {
        Some(parent) => collect_category_path(parent),
        None => Vec::new(),
    };
    path.push(category.value.clone());
    path
}

fn extras_to_map(extras: Vec<ApiExtra>) -> BTreeMap<String, Vec<String>> {
    extras
        .into_iter()
        .filter_map(|extra| {
            if extra.id.trim().is_empty() || extra.values.is_empty() {
                None
            } else {
                Some((extra.id, extra.values))
            }
        })
        .collect()
}

fn flatten_category_entries(
    items: &[ApiFilterItem],
    parent_path: &mut Vec<String>,
    out: &mut Vec<CategoryEntry>,
) {
    for item in items {
        parent_path.push(item.display_name.clone());
        out.push(CategoryEntry {
            label: item.display_name.clone(),
            path: parent_path.clone(),
            query_param: item.name.clone(),
            value: item.value.clone(),
        });
        flatten_category_entries(&item.filter_items, parent_path, out);
        parent_path.pop();
    }
}

#[derive(Debug, Deserialize)]
struct ApiCategoriesResponse {
    #[serde(default)]
    filters: Vec<ApiFilter>,
}

#[derive(Debug, Deserialize)]
struct ApiFilter {
    name: String,
    #[serde(default)]
    filter_items: Vec<ApiFilterItem>,
}

#[derive(Debug, Deserialize)]
struct ApiFilterItem {
    display_name: String,
    name: String,
    value: String,
    #[serde(default)]
    filter_items: Vec<ApiFilterItem>,
}

#[derive(Debug, Deserialize)]
struct ApiSearchResponse {
    docs: Vec<ApiItem>,
    metadata: ApiMetadata,
}

#[derive(Debug, Deserialize)]
struct ApiMetadata {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    search_key_description: Option<String>,
    paging: ApiPaging,
    result_size: ApiResultSize,
    #[serde(default)]
    is_end_of_paging: bool,
}

#[derive(Debug, Deserialize)]
struct ApiPaging {
    current: u32,
    last: u32,
}

#[derive(Debug, Deserialize)]
struct ApiResultSize {
    match_count: u64,
}

#[derive(Debug, Deserialize)]
struct ApiItem {
    id: String,
    #[serde(default)]
    ad_id: Option<u64>,
    heading: String,
    #[serde(default)]
    location: Option<String>,
    canonical_url: String,
    #[serde(default)]
    trade_type: Option<String>,
    #[serde(default)]
    price: Option<ApiPrice>,
    #[serde(default)]
    timestamp: Option<i64>,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    labels: Vec<ApiLabel>,
    #[serde(default)]
    image: Option<ApiImage>,
    #[serde(default)]
    image_urls: Vec<String>,
    #[serde(default)]
    brand: Option<String>,
    #[serde(default)]
    coordinates: Option<ApiCoordinates>,
    #[serde(default)]
    extras: Vec<ApiExtra>,
}

#[derive(Debug, Deserialize)]
struct ApiPrice {
    amount: u64,
    currency_code: String,
    price_unit: String,
}

#[derive(Debug, Deserialize)]
struct ApiLabel {
    text: String,
}

#[derive(Debug, Deserialize)]
struct ApiImage {
    url: String,
}

#[derive(Debug, Deserialize)]
struct ApiCoordinates {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct ApiExtra {
    id: String,
    values: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ItemHydrationRoot {
    #[serde(rename = "loaderData")]
    loader_data: ItemLoaderData,
}

#[derive(Debug, Deserialize)]
struct ItemLoaderData {
    #[serde(rename = "item-recommerce")]
    item_recommerce: ApiItemRoute,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiItemRoute {
    item_data: ApiItemData,
    meta: ApiItemPageMeta,
    #[serde(default)]
    json_ld: Option<ApiJsonLd>,
    #[serde(default)]
    transactable_data: Option<ApiTransactableData>,
}

#[derive(Debug, Deserialize)]
struct ApiItemPageMeta {
    #[serde(default)]
    canonical: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiJsonLd {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    offers: Option<ApiJsonLdOffer>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiJsonLdOffer {
    price_currency: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiItemData {
    title: String,
    #[serde(default)]
    location: Option<ApiItemLocation>,
    #[serde(default)]
    extras: Vec<ApiItemExtra>,
    #[serde(default)]
    anonymous: bool,
    meta: ApiItemMeta,
    #[serde(default)]
    price: Option<u64>,
    #[serde(default)]
    category: Option<ApiCategory>,
    #[serde(default)]
    images: Vec<ApiItemImage>,
    #[serde(default)]
    disposed: bool,
    #[serde(default)]
    ad_view_type_label: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    is_webstore: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiItemLocation {
    #[serde(default)]
    position: Option<ApiItemPosition>,
    #[serde(default)]
    postal_code: Option<String>,
    #[serde(default)]
    postal_name: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
    #[serde(default)]
    country_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiItemPosition {
    lat: f64,
    #[serde(rename = "lng")]
    lng: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiItemExtra {
    id: String,
    #[serde(default)]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiItemMeta {
    ad_id: String,
    #[serde(default)]
    edited: Option<String>,
    #[serde(default)]
    is_inactive: bool,
}

#[derive(Debug, Deserialize)]
struct ApiCategory {
    value: String,
    #[serde(default)]
    parent: Option<Box<ApiCategory>>,
}

#[derive(Debug, Deserialize)]
struct ApiItemImage {
    uri: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ApiTransactableData {
    #[serde(default)]
    transactable: bool,
    #[serde(default)]
    seller_pays_shipping: bool,
    #[serde(default)]
    buy_now: bool,
    #[serde(default)]
    eligible_for_shipping: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        ApiCategoriesResponse, ApiSearchResponse, build_search_query, extract_item_hydration,
        extract_item_id, flatten_category_entries, map_item_detail, map_search_response,
        normalize_category, parse_category_id, resolve_category_name, suggest_categories,
    };
    use crate::model::{
        CategoryEntry, ConditionFilter, DealerSegmentFilter, ForRentFilter, SearchParams,
        SearchSort, TradeTypeFilter,
    };

    fn fixture_categories() -> Vec<CategoryEntry> {
        let fixture = include_str!("../tests/fixtures/categories_response.json");
        let api: ApiCategoriesResponse =
            serde_json::from_str(fixture).expect("fixture to deserialize");
        let filter = api
            .filters
            .into_iter()
            .find(|filter| filter.name == "category")
            .expect("category filter to exist");
        let mut categories = Vec::new();
        flatten_category_entries(&filter.filter_items, &mut Vec::new(), &mut categories);
        categories
    }

    #[test]
    fn builds_search_query_with_filters() {
        let category = CategoryEntry {
            label: "Elektronikk og hvitevarer".to_string(),
            path: vec!["Elektronikk og hvitevarer".to_string()],
            query_param: "category".to_string(),
            value: "0.93".to_string(),
        };
        let params = SearchParams::new("rtx 4080")
            .with_category("Elektronikk og hvitevarer")
            .with_page(2)
            .with_sort(SearchSort::PriceAsc)
            .with_price_from(10_000)
            .with_price_to(20_000)
            .with_shipping(true)
            .with_location("0.20061")
            .with_for_rent(ForRentFilter::Buy)
            .with_trade_type(TradeTypeFilter::ForSale)
            .with_dealer_segment(DealerSegmentFilter::Private)
            .with_condition(ConditionFilter::LikeNew)
            .with_published_today(true)
            .with_raw_param("foo", "bar");

        let query = build_search_query(&params, 2, Some(&category));

        assert!(query.contains(&("q".to_string(), "rtx 4080".to_string())));
        assert!(query.contains(&("page".to_string(), "2".to_string())));
        assert!(query.contains(&("category".to_string(), "0.93".to_string())));
        assert!(query.contains(&("sort".to_string(), "PRICE_ASC".to_string())));
        assert!(query.contains(&("price_from".to_string(), "10000".to_string())));
        assert!(query.contains(&("price_to".to_string(), "20000".to_string())));
        assert!(query.contains(&("shipping_types".to_string(), "0".to_string())));
        assert!(query.contains(&("location".to_string(), "0.20061".to_string())));
        assert!(query.contains(&("for_rent".to_string(), "0".to_string())));
        assert!(query.contains(&("trade_type".to_string(), "1".to_string())));
        assert!(query.contains(&("dealer_segment".to_string(), "1".to_string())));
        assert!(query.contains(&("condition".to_string(), "2".to_string())));
        assert!(query.contains(&("published".to_string(), "1".to_string())));
        assert!(query.contains(&("foo".to_string(), "bar".to_string())));
    }

    #[test]
    fn parses_fixture_and_maps_search_items() {
        let fixture = include_str!("../tests/fixtures/search_response.json");
        let api: ApiSearchResponse = serde_json::from_str(fixture).expect("fixture to deserialize");
        let params = SearchParams::new("rtx 4080")
            .with_category("0.93")
            .with_page(2);

        let result = map_search_response(&params, api).expect("response to map");

        assert_eq!(result.current_page, 2);
        assert_eq!(result.last_page, 2);
        assert_eq!(result.fetched_pages, 1);
        assert_eq!(result.total_matches, 78);
        assert_eq!(result.returned_items, 2);
        assert_eq!(result.items[0].id, 436749637);
        assert_eq!(result.items[1].brand.as_deref(), Some("Asus"));
        assert_eq!(
            result.items[1].extras.get("brand").expect("brand extra"),
            &vec!["Asus".to_string()]
        );
    }

    #[test]
    fn extracts_item_detail_from_fixture_html() {
        let fixture = include_str!("../tests/fixtures/item_page.html");
        let hydration = extract_item_hydration(fixture).expect("hydration data to parse");
        let item = map_item_detail(
            hydration,
            "https://www.finn.no/recommerce/forsale/item/451260160",
        )
        .expect("item detail to map");

        assert_eq!(item.id, 451260160);
        assert_eq!(item.price.as_ref().map(|price| price.amount), Some(12_000));
        assert_eq!(item.location.as_deref(), Some("4842 Arendal"));
        assert_eq!(
            item.category_path.join(" > "),
            "Elektronikk og hvitevarer > Data > Datakomponenter"
        );
        assert!(item.buy_now);
        assert!(item.eligible_for_shipping);
        assert_eq!(
            item.extras.get("condition").map(String::as_str),
            Some("Pent brukt - I god stand")
        );
    }

    #[test]
    fn flattens_and_resolves_categories_by_name_and_path() {
        let categories = fixture_categories();

        let top = resolve_category_name("Elektronikk og hvitevarer", &categories)
            .expect("top-level category to resolve");
        assert_eq!(top.query_param, "category");
        assert_eq!(top.value, "0.93");

        let nested = resolve_category_name(
            "Elektronikk og hvitevarer > Data > Datakomponenter",
            &categories,
        )
        .expect("nested category to resolve");
        assert_eq!(nested.query_param, "product_category");
        assert_eq!(nested.value, "2.93.3215.8368");
    }

    #[test]
    fn resolves_categories_by_alias_and_typo() {
        let categories = fixture_categories();

        let alias = resolve_category_name("electronics", &categories).expect("alias to resolve");
        assert_eq!(alias.value, "0.93");

        let fuzzy = resolve_category_name("datakomponnter", &categories).expect("typo to resolve");
        assert_eq!(fuzzy.value, "2.93.3215.8368");
    }

    #[test]
    fn suggests_categories_for_picker() {
        let categories = fixture_categories();
        let suggested = suggest_categories(&categories, "datakomponnter", 3);

        assert_eq!(
            suggested.first().map(|category| category.value.as_str()),
            Some("2.93.3215.8368")
        );
    }

    #[test]
    fn parses_category_id_prefixes() {
        assert_eq!(
            parse_category_id("0.93").map(|category| category.query_param),
            Some("category".to_string())
        );
        assert_eq!(
            parse_category_id("1.93.3215").map(|category| category.query_param),
            Some("sub_category".to_string())
        );
        assert_eq!(
            parse_category_id("2.93.3215.8368").map(|category| category.query_param),
            Some("product_category".to_string())
        );
    }

    #[test]
    fn normalizes_category_spacing_and_letters() {
        assert_eq!(
            normalize_category(" Elektronikk   og hvitevarer>Data "),
            "elektronikk og hvitevarer data"
        );
        assert_eq!(normalize_category("Bærbar PC"), "baerbar pc");
    }

    #[test]
    fn extracts_item_id_from_plain_id_and_url() {
        assert_eq!(extract_item_id("451260160").as_deref(), Some("451260160"));
        assert_eq!(
            extract_item_id("https://www.finn.no/recommerce/forsale/item/451260160").as_deref(),
            Some("451260160")
        );
    }
}
