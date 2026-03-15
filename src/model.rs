use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchParams {
    pub query: String,
    pub category: Option<String>,
    pub page: u32,
    pub sort: Option<SearchSort>,
    pub price_from: Option<u64>,
    pub price_to: Option<u64>,
    pub shipping: bool,
    pub locations: Vec<String>,
    pub for_rent: Option<ForRentFilter>,
    pub trade_type: Option<TradeTypeFilter>,
    pub dealer_segment: Option<DealerSegmentFilter>,
    pub conditions: Vec<ConditionFilter>,
    pub published_today: bool,
    pub raw_params: Vec<(String, String)>,
}

impl SearchParams {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into().trim().to_string(),
            category: None,
            page: 1,
            sort: None,
            price_from: None,
            price_to: None,
            shipping: false,
            locations: Vec::new(),
            for_rent: None,
            trade_type: None,
            dealer_segment: None,
            conditions: Vec::new(),
            published_today: false,
            raw_params: Vec::new(),
        }
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into().trim().to_string());
        self
    }

    pub fn with_page(mut self, page: u32) -> Self {
        self.page = page.max(1);
        self
    }

    pub fn with_sort(mut self, sort: SearchSort) -> Self {
        self.sort = Some(sort);
        self
    }

    pub fn with_price_from(mut self, price_from: u64) -> Self {
        self.price_from = Some(price_from);
        self
    }

    pub fn with_price_to(mut self, price_to: u64) -> Self {
        self.price_to = Some(price_to);
        self
    }

    pub fn with_shipping(mut self, shipping: bool) -> Self {
        self.shipping = shipping;
        self
    }

    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.locations.push(location.into().trim().to_string());
        self
    }

    pub fn with_for_rent(mut self, for_rent: ForRentFilter) -> Self {
        self.for_rent = Some(for_rent);
        self
    }

    pub fn with_trade_type(mut self, trade_type: TradeTypeFilter) -> Self {
        self.trade_type = Some(trade_type);
        self
    }

    pub fn with_dealer_segment(mut self, dealer_segment: DealerSegmentFilter) -> Self {
        self.dealer_segment = Some(dealer_segment);
        self
    }

    pub fn with_condition(mut self, condition: ConditionFilter) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn with_published_today(mut self, published_today: bool) -> Self {
        self.published_today = published_today;
        self
    }

    pub fn with_raw_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.raw_params.push((
            key.into().trim().to_string(),
            value.into().trim().to_string(),
        ));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum SearchSort {
    #[value(name = "oldest")]
    Oldest,
    #[value(name = "relevance")]
    Relevance,
    #[value(name = "newest")]
    Newest,
    #[value(name = "closest")]
    Closest,
    #[value(name = "price-desc")]
    PriceDesc,
    #[value(name = "price-asc")]
    PriceAsc,
}

impl SearchSort {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Oldest => "PUBLISHED_ASC",
            Self::Relevance => "RELEVANCE",
            Self::Newest => "PUBLISHED_DESC",
            Self::Closest => "CLOSEST",
            Self::PriceDesc => "PRICE_DESC",
            Self::PriceAsc => "PRICE_ASC",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum ForRentFilter {
    #[value(name = "buy")]
    Buy,
    #[value(name = "rent")]
    Rent,
}

impl ForRentFilter {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Buy => "0",
            Self::Rent => "1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum TradeTypeFilter {
    #[value(name = "for-sale")]
    ForSale,
    #[value(name = "give-away")]
    GiveAway,
    #[value(name = "wanted")]
    Wanted,
}

impl TradeTypeFilter {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::ForSale => "1",
            Self::GiveAway => "2",
            Self::Wanted => "3",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum DealerSegmentFilter {
    #[value(name = "private")]
    Private,
    #[value(name = "dealer")]
    Dealer,
}

impl DealerSegmentFilter {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::Private => "1",
            Self::Dealer => "3",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum ConditionFilter {
    #[value(name = "new")]
    New,
    #[value(name = "like-new")]
    LikeNew,
    #[value(name = "gently-used")]
    GentlyUsed,
    #[value(name = "well-used")]
    WellUsed,
    #[value(name = "needs-repair")]
    NeedsRepair,
}

impl ConditionFilter {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::New => "1",
            Self::LikeNew => "2",
            Self::GentlyUsed => "3",
            Self::WellUsed => "4",
            Self::NeedsRepair => "5",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryEntry {
    pub label: String,
    pub path: Vec<String>,
    pub query_param: String,
    pub value: String,
}

impl CategoryEntry {
    pub fn path_string(&self) -> String {
        self.path.join(" > ")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketplace: Option<String>,
    pub current_page: u32,
    pub last_page: u32,
    pub fetched_pages: u32,
    pub fetched_from_page: u32,
    pub fetched_to_page: u32,
    pub total_matches: u64,
    pub returned_items: usize,
    pub is_end_of_paging: bool,
    pub items: Vec<SearchItem>,
}

impl SearchResult {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchItem {
    pub id: u64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_type: Option<String>,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extras: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDetail {
    pub id: u64,
    pub title: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<Coordinates>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub category_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_urls: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extras: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_at: Option<String>,
    pub is_transactable: bool,
    pub buy_now: bool,
    pub eligible_for_shipping: bool,
    pub seller_pays_shipping: bool,
    pub is_webstore: bool,
    pub anonymous_seller: bool,
    pub is_inactive: bool,
    pub is_disposed: bool,
}

impl ItemDetail {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Price {
    pub amount: u64,
    pub currency_code: String,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coordinates {
    pub lat: f64,
    pub lon: f64,
}
