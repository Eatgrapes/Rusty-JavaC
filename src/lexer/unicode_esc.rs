pub fn unescape_unicode(src: &str) -> String {
    let mut result = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' || chars.peek() != Some(&'u') {
            result.push(c);
            continue;
        }

        chars.next();
        while chars.peek() == Some(&'u') {
            chars.next();
        }

        let mut hex = String::with_capacity(4);
        for _ in 0..4 {
            if let Some(h) = chars.next() {
                hex.push(h);
            }
        }

        if hex.len() == 4
            && let Ok(code) = u32::from_str_radix(&hex, 16)
            && let Some(ch) = char::from_u32(code)
        {
            result.push(ch);
            continue;
        }

        result.push('\\');
        result.push('u');
        for h in hex.chars() {
            result.push(h);
        }
    }

    result
}
