use std::collections::HashMap;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::naver::model::{Index, IndexQuotePage, MarketState};

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum ShareKind {
    Index,
    Stock,
}

pub(crate) struct Share {
    pub(crate) kind: ShareKind,
    pub(crate) name: String,
    pub(crate) state: MarketState,
    pub(crate) value: i64,
    pub(crate) change_value: i64,
    pub(crate) change_rate: f64,
    pub(crate) trading_volume: i64,
    pub(crate) graph: Graph,
}

pub(crate) struct Market {
    shares: HashMap<String, Share>,
}

impl Market {
    pub fn new() -> Self {
        Market {
            shares: HashMap::new(),
        }
    }

    pub fn share_codes(&self) -> Vec<&String> {
        self.shares.keys().collect()
    }

    pub fn share_codes_with_kind(&self) -> Vec<(&String, ShareKind)> {
        self.shares.iter().map(|(code, share)| (code, share.kind)).collect()
    }

    pub fn add_or_update_index(&mut self, code: &str, index: &Index) {
        let share = self.shares.get_mut(code);
        if let Some(share) = share {
            share.name = code.to_owned();
            share.state = index.state;
            share.value = index.now_value;
            share.change_value = index.change_value;
            share.change_rate = index.change_rate;
            share.trading_volume = index.trading_volume;
        } else {
            self.shares.insert(
                code.into(),
                Share {
                    kind: ShareKind::Index,
                    name: code.to_owned(),
                    state: index.state,
                    value: index.now_value,
                    change_value: index.change_value,
                    change_rate: index.change_rate,
                    trading_volume: index.trading_volume,
                    graph: Graph::new(),
                },
            );
        }
    }

    pub fn update_index_graph(&mut self, code: &str, page: &IndexQuotePage, date: &NaiveDate) {
        let share = self.shares.get_mut(code);
        if let Some(share) = share {
            for quote in &page.quotes {
                let time = NaiveTime::parse_from_str(&quote.time, "%H:%M");
                if let Ok(time) = time {
                    share.graph.update(Quote {
                        time: date.and_time(time),
                        value: (quote.value() * 100.0).round() as i64,
                        trading_volume: quote.trading_volume(),
                        trading_value: quote.trading_value(),
                    });
                }
            }
        }
    }

    pub fn get_share(&self, code: &str) -> Option<&Share> {
        self.shares.get(code)
    }

    pub fn remove_share(&mut self, code: &str) -> Option<Share> {
        self.shares.remove(code)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Quote {
    time: NaiveDateTime,
    value: i64,
    trading_volume: i64,
    trading_value: i64,
}

pub(crate) struct Graph {
    quotes: Vec<Quote>,
}

impl Graph {
    const MAX_QUOTES: usize = 1024;

    fn new() -> Self {
        Graph { quotes: Vec::new() }
    }

    fn update(&mut self, quote: Quote) {
        let pos = self.quotes.binary_search_by_key(&quote.time, |q| q.time);
        match pos {
            Ok(pos) => self.quotes[pos] = quote,
            Err(pos) => self.quotes.insert(pos, quote),
        }

        if self.quotes.len() > Graph::MAX_QUOTES {
            self.quotes.remove(0);
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.quotes.len()
    }
}
