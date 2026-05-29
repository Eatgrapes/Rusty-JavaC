use ustr::Ustr;

pub(super) fn parse_int_literal(text: &str) -> i64 {
    parse_integer_digits(text.trim_end_matches(['l', 'L']))
}

pub(super) fn parse_long_literal(text: &str) -> i64 {
    parse_integer_digits(text.trim_end_matches(['l', 'L']))
}

pub(super) fn parse_float_literal(text: &str) -> f32 {
    text.trim_end_matches(['f', 'F'])
        .replace('_', "")
        .parse()
        .unwrap_or(0.0)
}

pub(super) fn parse_double_literal(text: &str) -> f64 {
    text.trim_end_matches(['d', 'D'])
        .replace('_', "")
        .parse()
        .unwrap_or(0.0)
}

pub(super) fn has_float_suffix(text: &str) -> bool {
    text.ends_with(['f', 'F'])
}

pub(super) fn parse_char_literal(text: &str) -> char {
    let value = text
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .unwrap_or(text);
    match value {
        "\\n" => '\n',
        "\\t" => '\t',
        "\\r" => '\r',
        "\\'" => '\'',
        "\\\\" => '\\',
        _ => value.chars().next().unwrap_or('\0'),
    }
}

pub(super) fn string_literal_value(text: &str) -> Ustr {
    let unquoted = text
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(text)
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\");
    Ustr::from(&unquoted)
}

fn parse_integer_digits(text: &str) -> i64 {
    let cleaned = text.replace('_', "");
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).unwrap_or(0)
    } else if let Some(binary) = cleaned
        .strip_prefix("0b")
        .or_else(|| cleaned.strip_prefix("0B"))
    {
        i64::from_str_radix(binary, 2).unwrap_or(0)
    } else {
        cleaned.parse().unwrap_or(0)
    }
}
