use std::fmt::Display;

use serde::{Deserialize, Serialize};
use unhtml_derive::FromHtml;

use detail::CommaNumber;

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum MarketState {
    #[serde(rename = "PREOPEN")]
    PreOpen,
    #[serde(rename = "CLOSE")]
    Close,
    #[serde(rename = "OPEN")]
    Open,
}

impl Display for MarketState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::PreOpen => "장전",
            Self::Close => "장마감",
            Self::Open => "장중",
        };
        write!(f, "{}", text)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Index {
    /// 장 상태.
    #[serde(rename = "ms")]
    pub state: MarketState,

    /// 현재가(0.01P).
    #[serde(rename = "nv")]
    pub now_value: i64,

    /// 장중최고(0.01원).
    #[serde(rename = "hv")]
    pub high_value: i64,

    /// 장중최저(0.01원).
    #[serde(rename = "lv")]
    pub low_value: i64,

    /// 등락폭(0.01원).
    #[serde(rename = "cv")]
    pub change_value: i64,

    /// 등락률(%).
    #[serde(rename = "cr")]
    pub change_rate: f64,

    /// 거래량(1000주).
    #[serde(rename = "aq")]
    pub trading_volume: i64,

    /// 거래대금(백만원).
    #[serde(rename = "aa")]
    pub trading_value: i64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Stock {
    /// 이름.
    #[serde(rename = "nm")]
    pub name: String,

    /// 장 상태.
    #[serde(rename = "ms")]
    pub state: MarketState,

    /// 현재가(1원).
    #[serde(rename = "nv")]
    pub now_value: i64,

    /// 장중최고(1원).
    #[serde(rename = "hv")]
    pub high_value: i64,

    /// 장중최저(1원).
    #[serde(rename = "lv")]
    pub low_value: i64,

    /// 등락종류(1 ~ 5 : 상한가, 상승, 보합, 하한가, 하락).
    #[serde(rename = "rf")]
    pub(super) change_type: String,

    /// 등락폭 절댓값(1원).
    #[serde(rename = "cv")]
    pub(super) change_value: i64,

    /// 등락률 절댓값(%).
    #[serde(rename = "cr")]
    pub(super) change_rate: f64,

    /// 거래량(1주).
    #[serde(rename = "aq")]
    pub trading_volume: i64,

    /// 거래대금(1원).
    #[serde(rename = "aa")]
    pub trading_value: i64,
}

impl Stock {
    /// 등락폭(1원).
    pub fn change_value(&self) -> i64 {
        if self.change_type == "4" || self.change_type == "5" {
            -self.change_value
        } else {
            self.change_value
        }
    }

    /// 등락률(%).
    pub fn change_rate(&self) -> f64 {
        if self.change_type == "4" || self.change_type == "5" {
            -self.change_rate
        } else {
            self.change_rate
        }
    }
}

#[derive(Debug, PartialEq, FromHtml)]
#[html(selector = "tr")]
struct IndexQuote {
    /// 체결시각(HH:mm).
    #[html(selector = "td.date", attr = "inner")]
    pub time: String,

    /// 체결가(1P).
    #[html(selector = "td:nth-child(2)", attr = "inner")]
    pub value: CommaNumber<f64>,

    /// 거래량(1000주).
    #[html(selector = "td:nth-child(5)", attr = "inner")]
    pub trading_volume: CommaNumber<i64>,

    /// 거래대금(백만원).
    #[html(selector = "td:nth-child(6)", attr = "inner")]
    pub trading_value: CommaNumber<i64>,
}

mod detail {
    use std::str::FromStr;

    #[derive(Debug, PartialEq)]
    pub(super) struct CommaNumber<T>(T);

    impl<T> From<T> for CommaNumber<T>
    where
        T: FromStr,
    {
        fn from(val: T) -> Self {
            CommaNumber(val)
        }
    }

    impl<T> unhtml::FromText for CommaNumber<T>
    where
        T: FromStr,
    {
        fn from_inner_text(select: unhtml::ElemIter) -> unhtml::Result<Self> {
            let first = select.next().ok_or(())?;
            let mut ret = String::new();
            for next_segment in first.text() {
                ret += next_segment.trim();
            }
            T::from_str(&ret.replace(',', ""))
                .map(CommaNumber)
                .map_err(|_| unhtml::Error::TextParseError {
                    text: ret,
                    type_name: "CommaNumber".into(),
                    err: "TextParseError".into(),
                })
        }

        fn from_attr(select: unhtml::ElemIter, attr: &str) -> unhtml::Result<Self> {
            let first = select.next().ok_or(())?;
            let attr = first
                .value()
                .attr(attr)
                .ok_or((attr.to_owned(), first.html()))?;
            T::from_str(&attr.trim().replace(',', ""))
                .map(CommaNumber)
                .map_err(|_| unhtml::Error::TextParseError {
                    text: attr.trim().into(),
                    type_name: "CommaNumber".into(),
                    err: "TextParseError".into(),
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unhtml::FromHtml;

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

    #[test]
    fn parse_stock() {
        let data = r#" {"cd":"005930","nm":"삼성전자","sv":58800,"nv":58500,"cv":300,"cr":0.51,"rf":"5","mt":"1","ms":"CLOSE","tyn":"N","pcv":58800,"ov":58900,"hv":59000,"lv":57800,"ul":76400,"ll":41200,"aq":21316295,"aa":1245504000000,"nav":null,"keps":3166,"eps":3196,"bps":38533.50654,"cnsEps":4083,"dv":1416.00000} "#;
        let stock: Stock = serde_json::from_str(data).unwrap();
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

    #[test]
    fn parse_index_cond() {
        let html = r#" <table><tr>
        <td class="date">15:32</td>
        <td class="number_1">2,343.31</td>
        <td class="rate_down" style="padding-right:35px;">
            <img src="..." width="7" height="6" style="margin-right:4px;" alt="상승">
            <span class="tah p11 red02">43.15</span>
        </td>
        <td class="number_1">333</td>
        <td class="number_1" style="padding-right:40px;">874,016</td>
        <td class="number_1" style="padding-right:30px;">10,692,707</td>
        </tr></table> "#;

        let cond = IndexQuote::from_html(html).unwrap();
        assert_eq!(
            cond,
            IndexQuote {
                time: "15:32".into(),
                value: 2343.31.into(),
                trading_volume: 874016.into(),
                trading_value: 10692707.into(),
            }
        );
    }
}
