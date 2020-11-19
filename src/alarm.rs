use std::collections::HashMap;

pub(crate) struct StockAlarm {
    alarms: HashMap<String, Vec<i64>>,
}

impl StockAlarm {
    pub fn new() -> Self {
        StockAlarm {
            alarms: HashMap::new(),
        }
    }

    pub fn set_alarm(&mut self, code: &str, target_value: i64) {
        self.alarms
            .entry(code.to_owned())
            .and_modify(|v| {
                // 이미 있는 알람이 아니면 정렬된 위치에 삽입.
                if let Err(i) = v.binary_search(&target_value) {
                    v.insert(i, target_value);
                }
            })
            .or_insert([target_value].to_vec());
    }

    pub fn remove_alarm(&mut self, code: &str, target_value: i64) -> bool {
        if let Some(v) = self.alarms.get_mut(code) {
            if let Ok(i) = v.binary_search(&target_value) {
                v.remove(i);
                if v.is_empty() {
                    self.alarms.remove(code);
                }
                
                return true
            }
        }

        false
    }

    pub fn codes(&self) -> Vec<&String> {
        self.alarms.keys().collect()
    }

    pub fn get_alarms(&self, code: &str) -> Option<&Vec<i64>> {
        self.alarms.get(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_empty_alarms() {
        let alarms = StockAlarm::new();
        assert_eq!(alarms.codes().len(), 0);
        assert!(alarms.get_alarms("").is_none());
    }

    #[test]
    fn set_and_remove_alarms() {
        let mut alarms = StockAlarm::new();

        // 알람 추가.
        alarms.set_alarm("code", 777);
        alarms.set_alarm("code", 42);
        assert_eq!(alarms.codes()[0], "code");
        assert_eq!(alarms.get_alarms("code"), Some(&vec![42, 777]));

        // 알람 제거.
        alarms.remove_alarm("code", 42);
        assert_eq!(alarms.get_alarms("code"), Some(&vec![777]));

        // 없는 알람 제거 시도.
        alarms.remove_alarm("code", -1);
        assert_eq!(alarms.get_alarms("code").unwrap().len(), 1);

        // 마지막 남은 알람 제거.
        alarms.remove_alarm("code", 777);
        assert!(alarms.get_alarms("code").is_none());
        assert_eq!(alarms.codes().len(), 0);
    }
}
