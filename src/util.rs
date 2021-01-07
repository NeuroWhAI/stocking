use std::cmp::Ordering;

use serenity::utils::Colour;

pub(crate) fn format_value(mut val: i64, radix: i64) -> String {
    let mut s = String::new();

    if val < 0 {
        val = -val;
        s.push('-');
    }

    let denominator = {
        let mut mul = 1;
        for _ in 0..radix {
            mul *= 10;
        }
        mul
    };

    let mut integer = val / denominator;
    let mut base = 1_000_000_000_000_000_000;
    let mut digit = 0;

    while base > integer {
        base /= 10;
        digit += 1;
    }

    while base >= 10 {
        if !s.is_empty() && &s[..1] != "-" && digit % 3 == 1 {
            s.push(',');
        }
        s.push((integer / base + '0' as i64) as u8 as char);

        integer -= (integer / base) * base;
        base /= 10;
        digit += 1;
    }

    s.push((integer + '0' as i64) as u8 as char);

    if radix > 0 {
        s.push('.');
        s.push_str(&format!("{:01$}", val % denominator, radix as usize));
    }

    s
}

pub(crate) fn get_change_value_char(val: i64) -> char {
    match val.cmp(&0) {
        Ordering::Greater => '▲',
        Ordering::Less => '▼',
        Ordering::Equal => '=',
    }
}

pub(crate) fn get_change_value_color<T>(val: T) -> Colour
where
    T: PartialOrd + From<i32>,
{
    if val > From::from(0) {
        Colour::from_rgb(217, 4, 0)
    } else if val < From::from(0) {
        Colour::from_rgb(0, 93, 222)
    } else {
        Colour::from_rgb(51, 51, 51)
    }
}

pub(crate) fn get_light_change_color<T>(val: T) -> Colour
where
    T: PartialOrd + From<i32>,
{
    if val > From::from(0) {
        Colour::from_rgb(239, 83, 80)
    } else if val < From::from(0) {
        Colour::from_rgb(92, 107, 192)
    } else {
        Colour::from_rgb(117, 117, 117)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_value_sets() {
        assert_eq!(format_value(0, 0), "0");
        assert_eq!(format_value(0, 1), "0.0");

        assert_eq!(format_value(1, 0), "1");
        assert_eq!(format_value(1, 1), "0.1");
        assert_eq!(format_value(10, 1), "1.0");
        assert_eq!(format_value(100, 2), "1.00");

        assert_eq!(format_value(-1, 0), "-1");
        assert_eq!(format_value(-1, 1), "-0.1");
        assert_eq!(format_value(-10, 1), "-1.0");
        assert_eq!(format_value(-100, 2), "-1.00");

        assert_eq!(format_value(321, 0), "321");
        assert_eq!(format_value(4321, 0), "4,321");
        assert_eq!(format_value(54321, 1), "5,432.1");
        assert_eq!(format_value(654321, 2), "6,543.21");

        assert_eq!(format_value(604301, 2), "6,043.01");
        assert_eq!(format_value(900604301, 2), "9,006,043.01");

        assert_eq!(
            format_value(9223372036854775807, 0),
            "9,223,372,036,854,775,807"
        );
    }
}
