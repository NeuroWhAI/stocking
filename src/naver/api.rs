use anyhow::{bail, Result};
use chrono::NaiveDateTime;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use unhtml::FromHtml;

use super::model::*;

const HOST_POLL: &str = "https://polling.finance.naver.com/";
const HOST_FINANCE: &str = "https://finance.naver.com/";
const HOST_M_STOCK: &str = "https://m.stock.naver.com/";

pub async fn get_index(name: &str) -> Result<Index> {
    let json: Value = request_url(&format!(
        "{}api/realtime?query=SERVICE_INDEX:{}",
        HOST_POLL, name
    ))
    .await?
    .json()
    .await?;

    Ok(parse_response(json, path_poll)?)
}

pub async fn get_stock(code: &str) -> Result<Stock> {
    let text = request_url(&format!(
        "{}api/realtime?query=SERVICE_ITEM:{}",
        HOST_POLL, code
    ))
    .await?
    .text_with_charset("euc-kr")
    .await?;

    let json = serde_json::from_str(&text)?;

    Ok(parse_response(json, path_poll)?)
}

pub async fn get_index_quotes(
    name: &str,
    date_and_max_time: &NaiveDateTime,
    page: usize,
) -> Result<IndexQuotePage> {
    let html = request_url(&format!(
        "{}sise/sise_index_time.nhn?code={}&thistime={}&page={}",
        HOST_FINANCE,
        name,
        date_and_max_time.format("%Y%m%d%H%M%S"),
        page
    ))
    .await?
    .text_with_charset("euc-kr")
    .await?;

    let page = IndexQuotePageOpt::from_html(&html)?;
    Ok(IndexQuotePage {
        quotes: page.quotes.into_iter().filter_map(|opt| opt).collect(),
        is_last: !html.contains("pgRR"),
    })
}

pub async fn get_stock_quotes(
    code: &str,
    date_and_max_time: &NaiveDateTime,
    page: usize,
) -> Result<StockQuotePage> {
    let html = request_url(&format!(
        "{}item/sise_time.nhn?code={}&thistime={}&page={}",
        HOST_FINANCE,
        code,
        date_and_max_time.format("%Y%m%d%H%M%S"),
        page
    ))
    .await?
    .text_with_charset("euc-kr")
    .await?;

    let page = StockQuotePageOpt::from_html(&html)?;
    Ok(StockQuotePage {
        quotes: page.quotes.into_iter().filter_map(|opt| opt).collect(),
        is_last: !html.contains("pgRR"),
    })
}

pub async fn search(keyword: &str) -> Result<Vec<SearchResult>> {
    let text = request_url(&format!(
        "{}api/json/search/searchListJson.nhn?keyword={}",
        HOST_M_STOCK, keyword
    ))
    .await?
    .text_with_charset("euc-kr")
    .await?;

    let json = serde_json::from_str(&text)?;

    Ok(parse_response(json, path_mobile_stock)?)
}

async fn request_url(url: &str) -> reqwest::Result<reqwest::Response> {
    let client = reqwest::Client::new();
    client
        .get(url)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_3)")
        .send()
        .await
}

fn parse_response<T, F>(mut json: Value, path: F) -> Result<T>
where
    T: DeserializeOwned,
    F: FnOnce(Option<&mut Value>) -> Option<Value>,
{
    if json["resultCode"] == json!("success") {
        let data = path(json.get_mut("result"));

        match data {
            Some(Value::Null) => bail!("{}", json["resultCode"]),
            Some(val) => Ok(serde_json::from_value(val)?),
            None => bail!("data path not exists"),
        }
    } else {
        bail!("{}", json["resultCode"])
    }
}

fn path_poll(json: Option<&mut Value>) -> Option<Value> {
    json.and_then(|v| v.get_mut("areas"))
        .and_then(|v| v.get_mut(0))
        .and_then(|v| v.get_mut("datas"))
        .and_then(|v| v.get_mut(0))
        .map(|v| v.take())
}

fn path_mobile_stock(json: Option<&mut Value>) -> Option<Value> {
    json.and_then(|v| v.get_mut("d")).map(|v| v.take())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn parse_index_success() {
        let data = r#" {"resultCode":"success","result":{"pollingInterval":50000,"areas":[{"name":"SERVICE_INDEX","datas":[{"ms":"CLOSE","nv":234526,"cv":1442,"cr":0.62,"hv":234546,"lv":231647,"aq":705770,"aa":8941027,"bs":0,"cd":"KOSPI"}]}],"time":1603889630919}} "#;
        let res: Result<Index> = parse_response(serde_json::from_str(data).unwrap(), path_poll);
        assert_eq!(
            res.unwrap(),
            Index {
                state: MarketState::Close,
                now_value: 234526,
                high_value: 234546,
                low_value: 231647,
                change_value: 1442,
                change_rate: 0.62,
                trading_volume: 705770,
                trading_value: 8941027,
            }
        );
    }

    #[test]
    fn parse_index_fail_result() {
        let data = r#" {"resultCode":"nope"} "#;
        let res: Result<Index> = parse_response(serde_json::from_str(data).unwrap(), path_poll);
        assert!(res.is_err());
    }

    #[test]
    fn parse_index_fail_no_data() {
        let data = r#" {"resultCode":"success","result":{"pollingInterval":50000,"areas":[{"name":"SERVICE_INDEX"}],"time":1603889630919}} "#;
        let res: Result<Index> = parse_response(serde_json::from_str(data).unwrap(), path_poll);
        assert!(res.is_err());
    }

    #[test]
    fn parse_stock_success() {
        let data = r#" {"resultCode":"success","result":{"pollingInterval":50000,"areas":[{"name":"SERVICE_ITEM","datas":[{"cd":"005930","nm":"삼성전자","sv":58800,"nv":58500,"cv":300,"cr":0.51,"rf":"5","mt":"1","ms":"CLOSE","tyn":"N","pcv":58800,"ov":58900,"hv":59000,"lv":57800,"ul":76400,"ll":41200,"aq":21316295,"aa":1245504000000,"nav":null,"keps":3166,"eps":3196,"bps":38533.50654,"cnsEps":4083,"dv":1416.00000}]}],"time":1604488004492}} "#;
        let stock: Stock = parse_response(serde_json::from_str(data).unwrap(), path_poll).unwrap();
        assert_eq!(
            stock,
            Stock {
                name: "삼성전자".into(),
                state: MarketState::Close,
                now_value: 58500,
                high_value: 59000,
                low_value: 57800,
                change_type: "5".into(),
                change_value: 300,
                change_rate: 0.51,
                trading_volume: 21316295,
                trading_value: 1245504000000,
            }
        );
        assert_eq!(stock.change_value(), -300);
        assert_approx_eq!(stock.change_rate(), -0.51);
    }

    #[test]
    fn parse_search_results() {
        let data = r#" {"result":{"d":[{"cd":"005930","nm":"삼성전자","nv":"63200","cv":"2200","cr":"3.61","rf":"2","mks":3772903,"aa":1949718,"nation":"KOR","etf":false},{"cd":"005935","nm":"삼성전자우","nv":"57400","cv":"100","cr":"0.17","rf":"2","mks":472337,"aa":180992,"nation":"KOR","etf":false},{"cd":"009150","nm":"삼성전기","nv":"150500","cv":"7000","cr":"4.88","rf":"2","mks":112414,"aa":253086,"nation":"KOR","etf":false},{"cd":"009155","nm":"삼성전기우","nv":"66500","cv":"3500","cr":"5.56","rf":"2","mks":1933,"aa":5694,"nation":"KOR","etf":false}],"totCnt":4,"t":"search"},"resultCode":"success"} "#;
        let results: Vec<SearchResult> =
            parse_response(serde_json::from_str(data).unwrap(), path_mobile_stock).unwrap();
        assert_eq!(results.len(), 4);
        assert_eq!(
            results[0],
            SearchResult {
                code: "005930".into(),
                name: "삼성전자".into(),
            }
        );
        assert_eq!(
            results[2],
            SearchResult {
                code: "009150".into(),
                name: "삼성전기".into(),
            }
        );
    }

    #[test]
    fn parse_search_fail_result() {
        let data = r#" {"resultCode":"nope"} "#;
        let res: Result<Vec<SearchResult>> =
            parse_response(serde_json::from_str(data).unwrap(), path_mobile_stock);
        assert!(res.is_err());
    }

    #[test]
    fn parse_search_fail_no_data() {
        let data = r#" {"resultCode":"success","result":{"nope":[]}} "#;
        let res: Result<Vec<SearchResult>> =
            parse_response(serde_json::from_str(data).unwrap(), path_mobile_stock);
        assert!(res.is_err());
    }
}
