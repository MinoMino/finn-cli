use crate::model::{CategoryEntry, ItemDetail, Price, SearchResult};
use std::fmt::Write;

pub fn render_search_result(result: &SearchResult) -> String {
    let mut out = String::new();

    let heading = result.title.as_deref().unwrap_or(result.query.as_str());

    writeln!(&mut out, "Search: {heading}").expect("write to string");
    writeln!(
        &mut out,
        "Matches: {} | Page {}/{} | Showing {} items",
        format_number(result.total_matches),
        result.current_page,
        result.last_page,
        result.returned_items
    )
    .expect("write to string");

    if result.fetched_pages > 1 {
        writeln!(
            &mut out,
            "Fetched pages: {}-{} ({} total)",
            result.fetched_from_page, result.fetched_to_page, result.fetched_pages
        )
        .expect("write to string");
    }

    if let Some(marketplace) = &result.marketplace {
        writeln!(&mut out, "Marketplace: {marketplace}").expect("write to string");
    }

    for (index, item) in result.items.iter().enumerate() {
        let price = item
            .price
            .as_ref()
            .map(format_price)
            .unwrap_or_else(|| "Unknown price".to_string());
        let location = item.location.as_deref().unwrap_or("Unknown location");
        let trade_type = item.trade_type.as_deref().unwrap_or("Unknown trade type");

        writeln!(&mut out, "\n{}. {}", index + 1, item.title).expect("write to string");
        writeln!(&mut out, "   {price} | {location} | {trade_type}").expect("write to string");
        writeln!(&mut out, "   {}", item.url).expect("write to string");

        let mut details = Vec::new();
        if let Some(published_at) = &item.published_at {
            details.push(format!("published {published_at}"));
        }
        if !item.labels.is_empty() {
            details.push(format!("labels: {}", item.labels.join(", ")));
        }
        if !item.flags.is_empty() {
            details.push(format!("flags: {}", item.flags.join(", ")));
        }
        if let Some(brand) = &item.brand {
            details.push(format!("brand: {brand}"));
        }

        if !details.is_empty() {
            writeln!(&mut out, "   {}", details.join(" | ")).expect("write to string");
        }
    }

    out.trim_end().to_string()
}

pub fn render_item_detail(item: &ItemDetail) -> String {
    let mut out = String::new();

    writeln!(&mut out, "Item: {}", item.title).expect("write to string");

    let mut summary = Vec::new();
    if let Some(price) = &item.price {
        summary.push(format_price(price));
    }
    if let Some(location) = &item.location {
        summary.push(location.clone());
    }
    if let Some(trade_type) = &item.trade_type {
        summary.push(trade_type.clone());
    }
    if !summary.is_empty() {
        writeln!(&mut out, "{}", summary.join(" | ")).expect("write to string");
    }

    writeln!(&mut out, "{}", item.url).expect("write to string");

    if !item.category_path.is_empty() {
        writeln!(&mut out, "Category: {}", item.category_path.join(" > "))
            .expect("write to string");
    }

    if let Some(edited_at) = &item.edited_at {
        writeln!(&mut out, "Edited: {edited_at}").expect("write to string");
    }

    let mut commerce = Vec::new();
    if item.is_transactable {
        commerce.push("transactable".to_string());
    }
    if item.buy_now {
        commerce.push("buy now".to_string());
    }
    if item.eligible_for_shipping {
        commerce.push("shipping available".to_string());
    }
    if item.seller_pays_shipping {
        commerce.push("seller pays shipping".to_string());
    }
    if !commerce.is_empty() {
        writeln!(&mut out, "Commerce: {}", commerce.join(" | ")).expect("write to string");
    }

    if !item.extras.is_empty() {
        writeln!(&mut out, "Attributes:").expect("write to string");
        for (key, value) in &item.extras {
            writeln!(&mut out, "  - {key}: {value}").expect("write to string");
        }
    }

    if let Some(description) = &item.description {
        writeln!(&mut out, "\nDescription:\n{description}").expect("write to string");
    }

    out.trim_end().to_string()
}

pub fn render_categories(categories: &[CategoryEntry]) -> String {
    let mut out = String::new();
    writeln!(&mut out, "Categories: {}", categories.len()).expect("write to string");

    for category in categories {
        writeln!(
            &mut out,
            "- {} ({}={})",
            category.path_string(),
            category.query_param,
            category.value
        )
        .expect("write to string");
    }

    out.trim_end().to_string()
}

fn format_price(price: &Price) -> String {
    format!("{} {}", format_number(price.amount), price.unit)
}

fn format_number(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, ch) in digits.chars().rev().enumerate() {
        if index != 0 && index % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }

    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::{render_categories, render_item_detail, render_search_result};
    use crate::model::{CategoryEntry, ItemDetail, Price, SearchItem, SearchResult};
    use std::collections::BTreeMap;

    #[test]
    fn renders_human_readable_search_output() {
        let result = SearchResult {
            query: "rtx 4080".to_string(),
            category: Some("0.93".to_string()),
            title: Some("rtx 4080, Elektronikk og hvitevarer".to_string()),
            marketplace: Some("Torget".to_string()),
            current_page: 1,
            last_page: 2,
            fetched_pages: 2,
            fetched_from_page: 1,
            fetched_to_page: 2,
            total_matches: 78,
            returned_items: 1,
            is_end_of_paging: true,
            items: vec![SearchItem {
                id: 455566626,
                title: "VANQUISHER iZ329X Gaming PC - RTX 4080 SUPER 16gb".to_string(),
                location: Some("Oslo".to_string()),
                price: Some(Price {
                    amount: 13500,
                    currency_code: "NOK".to_string(),
                    unit: "kr".to_string(),
                }),
                trade_type: Some("Til salgs".to_string()),
                url: "https://www.finn.no/recommerce/forsale/item/455566626".to_string(),
                primary_image_url: None,
                image_urls: vec![],
                labels: vec!["Fiks ferdig".to_string(), "Privat".to_string()],
                flags: vec!["private".to_string(), "shipping_exists".to_string()],
                brand: None,
                coordinates: None,
                timestamp_ms: Some(1773577066000),
                published_at: Some("2026-03-15T12:17:46+00:00".to_string()),
                extras: BTreeMap::new(),
            }],
        };

        let rendered = render_search_result(&result);

        assert!(rendered.contains("Search: rtx 4080, Elektronikk og hvitevarer"));
        assert!(rendered.contains("Matches: 78 | Page 1/2 | Showing 1 items"));
        assert!(rendered.contains("Fetched pages: 1-2 (2 total)"));
        assert!(rendered.contains("13 500 kr | Oslo | Til salgs"));
        assert!(rendered.contains("labels: Fiks ferdig, Privat"));
    }

    #[test]
    fn renders_item_detail_output() {
        let mut extras = BTreeMap::new();
        extras.insert(
            "condition".to_string(),
            "Pent brukt - I god stand".to_string(),
        );

        let item = ItemDetail {
            id: 451260160,
            title: "High end AMD Radeon RX 7900 XTX skjermkort 24 GB".to_string(),
            url: "https://www.finn.no/recommerce/forsale/item/451260160".to_string(),
            price: Some(Price {
                amount: 12000,
                currency_code: "NOK".to_string(),
                unit: "kr".to_string(),
            }),
            trade_type: Some("Til salgs".to_string()),
            description: Some("Kom med bud".to_string()),
            location: Some("4842 Arendal".to_string()),
            postal_code: Some("4842".to_string()),
            postal_name: Some("Arendal".to_string()),
            country_code: Some("NO".to_string()),
            country_name: Some("Norge".to_string()),
            coordinates: None,
            category_path: vec![
                "Elektronikk og hvitevarer".to_string(),
                "Data".to_string(),
                "Datakomponenter".to_string(),
            ],
            image_urls: vec![],
            extras,
            edited_at: Some("2026-03-10T12:16:05.387609+01:00".to_string()),
            is_transactable: true,
            buy_now: true,
            eligible_for_shipping: true,
            seller_pays_shipping: true,
            is_webstore: false,
            anonymous_seller: true,
            is_inactive: false,
            is_disposed: false,
        };

        let rendered = render_item_detail(&item);

        assert!(rendered.contains("Item: High end AMD Radeon RX 7900 XTX skjermkort 24 GB"));
        assert!(rendered.contains("12 000 kr | 4842 Arendal | Til salgs"));
        assert!(rendered.contains("Category: Elektronikk og hvitevarer > Data > Datakomponenter"));
        assert!(rendered.contains(
            "Commerce: transactable | buy now | shipping available | seller pays shipping"
        ));
        assert!(rendered.contains("- condition: Pent brukt - I god stand"));
    }

    #[test]
    fn renders_categories_output() {
        let categories = vec![CategoryEntry {
            label: "Datakomponenter".to_string(),
            path: vec![
                "Elektronikk og hvitevarer".to_string(),
                "Data".to_string(),
                "Datakomponenter".to_string(),
            ],
            query_param: "product_category".to_string(),
            value: "2.93.3215.8368".to_string(),
        }];

        let rendered = render_categories(&categories);
        assert!(rendered.contains("Categories: 1"));
        assert!(rendered.contains(
            "Elektronikk og hvitevarer > Data > Datakomponenter (product_category=2.93.3215.8368)"
        ));
    }
}
