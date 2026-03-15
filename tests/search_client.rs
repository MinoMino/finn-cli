use finn_cli::{
    ConditionFilter, DealerSegmentFilter, FinnClient, ForRentFilter, SearchParams, SearchSort,
    TradeTypeFilter,
};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn client_requests_search_endpoint_and_maps_response() {
    let server = MockServer::start().await;
    let fixture = include_str!("fixtures/search_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("category", "0.93"))
        .and(query_param("page", "2"))
        .and(query_param("sort", "PRICE_ASC"))
        .and(query_param("price_from", "10000"))
        .and(query_param("price_to", "20000"))
        .and(query_param("shipping_types", "0"))
        .and(query_param("location", "0.20061"))
        .and(query_param("for_rent", "0"))
        .and(query_param("trade_type", "1"))
        .and(query_param("dealer_segment", "1"))
        .and(query_param("condition", "2"))
        .and(query_param("published", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(fixture, "application/json"),
        )
        .mount(&server)
        .await;

    let client = FinnClient::with_base_url(server.uri()).expect("client to build");
    let params = SearchParams::new("rtx 4080")
        .with_category("0.93")
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
        .with_published_today(true);

    let result = client.search(&params).await.expect("search to succeed");

    assert_eq!(result.total_matches, 78);
    assert_eq!(result.current_page, 2);
    assert_eq!(result.last_page, 2);
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.items[0].id, 436749637);
    assert_eq!(result.items[1].brand.as_deref(), Some("Asus"));
}

#[tokio::test]
async fn client_can_resolve_category_name_to_id() {
    let server = MockServer::start().await;
    let categories = include_str!("fixtures/categories_response.json");
    let fixture = include_str!("fixtures/search_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param_is_missing("q"))
        .and(query_param_is_missing("page"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(categories, "application/json"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("product_category", "2.93.3215.8368"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(fixture, "application/json"),
        )
        .mount(&server)
        .await;

    let client = FinnClient::with_base_url(server.uri()).expect("client to build");
    let params = SearchParams::new("rtx 4080")
        .with_category("Elektronikk og hvitevarer > Data > Datakomponenter");

    let result = client.search(&params).await.expect("search to succeed");

    assert_eq!(result.total_matches, 78);
    assert_eq!(result.items.len(), 2);
}

#[tokio::test]
async fn client_can_resolve_category_alias() {
    let server = MockServer::start().await;
    let categories = include_str!("fixtures/categories_response.json");
    let fixture = include_str!("fixtures/search_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param_is_missing("q"))
        .and(query_param_is_missing("page"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(categories, "application/json"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("category", "0.93"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(fixture, "application/json"),
        )
        .mount(&server)
        .await;

    let client = FinnClient::with_base_url(server.uri()).expect("client to build");
    let params = SearchParams::new("rtx 4080").with_category("electronics");

    let result = client.search(&params).await.expect("search to succeed");
    assert_eq!(result.total_matches, 78);
}

#[tokio::test]
async fn client_can_fetch_all_pages() {
    let server = MockServer::start().await;
    let page_1 = include_str!("fixtures/search_response_page_1.json");
    let page_2 = include_str!("fixtures/search_response.json");

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("category", "0.93"))
        .and(query_param("page", "1"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(page_1, "application/json"),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/recommerce/forsale/search/api/search/SEARCH_ID_BAP_COMMON",
        ))
        .and(query_param("q", "rtx 4080"))
        .and(query_param("category", "0.93"))
        .and(query_param("page", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_raw(page_2, "application/json"),
        )
        .mount(&server)
        .await;

    let client = FinnClient::with_base_url(server.uri()).expect("client to build");
    let params = SearchParams::new("rtx 4080")
        .with_category("0.93")
        .with_page(1);

    let result = client
        .search_all(&params, None)
        .await
        .expect("search_all to succeed");

    assert_eq!(result.fetched_pages, 2);
    assert_eq!(result.fetched_from_page, 1);
    assert_eq!(result.fetched_to_page, 2);
    assert_eq!(result.returned_items, 4);
    assert!(result.is_end_of_paging);
}

#[tokio::test]
async fn client_can_fetch_item_detail() {
    let server = MockServer::start().await;
    let fixture = include_str!("fixtures/item_page.html");

    Mock::given(method("GET"))
        .and(path("/recommerce/forsale/item/451260160"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html; charset=utf-8")
                .set_body_string(fixture),
        )
        .mount(&server)
        .await;

    let client = FinnClient::with_base_url(server.uri()).expect("client to build");
    let item = client.get_item("451260160").await.expect("item to load");

    assert_eq!(item.id, 451260160);
    assert_eq!(item.location.as_deref(), Some("4842 Arendal"));
    assert!(item.buy_now);
}
