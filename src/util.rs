use serenity::utils::Colour;

pub(crate) fn format_value_with_base_100(mut val: i64) -> String {
    let mut s = String::new();

    if val < 0 {
        val = -val;
        s.push('-');
    }

    if val >= 100000 {
        s.push_str(&(val / 100000).to_string());
        s.push(',');
    }

    s.push_str(&(val % 100000 / 100).to_string());

    s.push('.');
    s.push_str(&(val % 100).to_string());

    s
}

pub(crate) fn get_change_value_char(val: i64) -> char {
    if val > 0 {
        '▲'
    } else if val < 0 {
        '▼'
    } else {
        '='
    }
}

pub(crate) fn get_change_value_color(val: i64) -> Colour {
    if val > 0 {
        Colour::from_rgb(217, 4, 0)
    } else if val < 0 {
        Colour::from_rgb(0, 93, 222)
    } else {
        Colour::from_rgb(51, 51, 51)
    }
}
