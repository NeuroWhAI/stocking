use anyhow::{bail, Result};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::model::*;

const HOST: &str = "https://polling.finance.naver.com/";

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

pub async fn get_index(name: &str) -> Result<Index> {
    let json: Value = reqwest::get(&format!(
        "{}api/realtime.nhn?query=SERVICE_INDEX:{}",
        HOST, name
    ))
    .await?
    .json()
    .await?;

    Ok(parse_response(json)?)
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
}
