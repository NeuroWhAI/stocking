use anyhow::{bail, Result};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use unhtml::FromHtml;
use chrono::NaiveDateTime;

use super::model::*;

const HOST_POLL: &str = "https://polling.finance.naver.com/";
const HOST_FINANCE: &str = "https://finance.naver.com/";

pub async fn get_index(name: &str) -> Result<Index> {
    let json: Value = reqwest::get(&format!(
        "{}api/realtime.nhn?query=SERVICE_INDEX:{}",
        HOST_POLL, name
    ))
    .await?
    .json()
    .await?;

    Ok(parse_response(json)?)
}

pub async fn get_stock(code: &str) -> Result<Stock> {
    let text = reqwest::get(&format!(
        "{}api/realtime.nhn?query=SERVICE_ITEM:{}",
        HOST_POLL, code
    ))
    .await?
    .text_with_charset("euc-kr")
    .await?;

    let json = serde_json::from_str(&text)?;

    Ok(parse_response(json)?)
}

pub async fn get_index_quotes(name: &str, date_and_max_time: &NaiveDateTime, page: usize) -> Result<IndexQuotePage> {
    let html = reqwest::get(&format!(
        "{}sise/sise_index_time.nhn?code={}&thistime={}&page={}",
        HOST_FINANCE, name, date_and_max_time.format("%Y%m%d%H%M%S"), page
    ))
    .await?
    .text_with_charset("euc-kr")
    .await?;

    let page = IndexQuotePageOpt::from_html(&html)?;
    Ok(IndexQuotePage {
        quotes: page.quotes
            .into_iter()
            .filter_map(|opt| opt)
            .collect(),
        is_last: !html.contains("pgRR"),
    })
}

pub async fn get_stock_quotes(code: &str, date_and_max_time: &NaiveDateTime, page: usize) -> Result<StockQuotePage> {
    let html = reqwest::get(&format!(
        "{}item/sise_time.nhn?code={}&thistime={}&page={}",
        HOST_FINANCE, code, date_and_max_time.format("%Y%m%d%H%M%S"), page
    ))
    .await?
    .text_with_charset("euc-kr")
    .await?;

    let page = StockQuotePageOpt::from_html(&html)?;
    Ok(StockQuotePage {
        quotes: page.quotes
            .into_iter()
            .filter_map(|opt| opt)
            .collect(),
        is_last: !html.contains("pgRR"),
    })
}

fn parse_response<T>(mut json: Value) -> Result<T>
where
    T: DeserializeOwned,
{
    if json["resultCode"] == json!("success") {
        let data = json
            .get_mut("result")
            .and_then(|v| v.get_mut("areas"))
            .and_then(|v| v.get_mut(0))
            .and_then(|v| v.get_mut("datas"))
            .and_then(|v| v.get_mut(0))
            .map(|v| v.take());

        match data {
            Some(Value::Null) => bail!("{}", json["resultCode"]),
            Some(val) => Ok(serde_json::from_value(val)?),
            None => bail!("data path not exists"),
        }
    } else {
        bail!("{}", json["resultCode"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_index_success() {
        let data = r#" {"resultCode":"success","result":{"pollingInterval":50000,"areas":[{"name":"SERVICE_INDEX","datas":[{"ms":"CLOSE","nv":234526,"cv":1442,"cr":0.62,"hv":234546,"lv":231647,"aq":705770,"aa":8941027,"bs":0,"cd":"KOSPI"}]}],"time":1603889630919}} "#;
        let res = parse_response::<Index>(serde_json::from_str(data).unwrap());
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
        let res = parse_response::<Index>(serde_json::from_str(data).unwrap());
        assert!(res.is_err());
    }

    #[test]
    fn parse_index_fail_no_data() {
        let data = r#" {"resultCode":"success","result":{"pollingInterval":50000,"areas":[{"name":"SERVICE_INDEX"}],"time":1603889630919}} "#;
        let res = parse_response::<Index>(serde_json::from_str(data).unwrap());
        assert!(res.is_err());
    }

    #[test]
    fn parse_stock_success() {
        let data = r#" {"resultCode":"success","result":{"pollingInterval":50000,"areas":[{"name":"SERVICE_ITEM","datas":[{"cd":"005930","nm":"삼성전자","sv":58800,"nv":58500,"cv":300,"cr":0.51,"rf":"5","mt":"1","ms":"CLOSE","tyn":"N","pcv":58800,"ov":58900,"hv":59000,"lv":57800,"ul":76400,"ll":41200,"aq":21316295,"aa":1245504000000,"nav":null,"keps":3166,"eps":3196,"bps":38533.50654,"cnsEps":4083,"dv":1416.00000}]}],"time":1604488004492}} "#;
        let stock = parse_response::<Stock>(serde_json::from_str(data).unwrap())
            .unwrap();
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
        assert_eq!(stock.change_rate(), -0.51);
    }
}
