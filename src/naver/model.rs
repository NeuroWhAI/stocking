use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum MarketState {
    #[serde(rename = "PREOPEN")]
    PreOpen,
    #[serde(rename = "CLOSE")]
    Close,
    #[serde(rename = "OPEN")]
    Open,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Index {
    /// 장 상태.
    #[serde(rename = "ms")]
    pub(crate) state: MarketState,

    /// 현재가(0.01원).
    #[serde(rename = "nv")]
    pub(crate) now_value: i64,

    /// 장중최고(0.01원).
    #[serde(rename = "hv")]
    pub(crate) high_value: i64,

    /// 장중최저(0.01원).
    #[serde(rename = "lv")]
    pub(crate) low_value: i64,

    /// 등락폭(0.01원).
    #[serde(rename = "cv")]
    pub(crate) change_value: i64,

    /// 등락률(%).
    #[serde(rename = "cr")]
    pub(crate) change_rate: f64,

    /// 거래량(1000주).
    #[serde(rename = "aq")]
    pub(crate) trading_volume: i64,

    /// 거래대금(백만원).
    #[serde(rename = "aa")]
    pub(crate) trading_value: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_market_state() {
        let data = r#" "PREOPEN" "#;
        let state: MarketState = serde_json::from_str(data).unwrap();
        assert_eq!(state, MarketState::PreOpen);

        let data = r#" "CLOSE" "#;
        let state: MarketState = serde_json::from_str(data).unwrap();
        assert_eq!(state, MarketState::Close);

        let data = r#" "OPEN" "#;
        let state: MarketState = serde_json::from_str(data).unwrap();
        assert_eq!(state, MarketState::Open);

        let data = r#" "NOPE" "#;
        assert!(serde_json::from_str::<MarketState>(data).is_err());
    }

    #[test]
    fn parse_index() {
        let data = r#" {"ms":"CLOSE","nv":234526,"cv":1442,"cr":0.62,"hv":234546,"lv":231647,"aq":705770,"aa":8941027,"bs":0,"cd":"KOSPI"} "#;
        let index: Index = serde_json::from_str(data).unwrap();
        assert_eq!(
            index,
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
}
