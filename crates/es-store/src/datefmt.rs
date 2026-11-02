use time::OffsetDateTime;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

fn num(n: i64, width: usize) -> String {
    format!("{n:0width$}")
}

fn name(full: &'static str, run: usize) -> &'static str {
    if run >= 4 { full } else { &full[..3] }
}

fn field(letter: char, run: usize, t: OffsetDateTime) -> Option<String> {
    let hour12 = match t.hour() % 12 {
        0 => 12,
        h => h,
    };
    Some(match letter {
        'y' if run == 2 => num(i64::from(t.year()).rem_euclid(100), 2),
        'y' => num(t.year().into(), run),
        'M' if run >= 3 => name(MONTHS[usize::from(u8::from(t.month())) - 1], run).into(),
        'M' => num(u8::from(t.month()).into(), run),
        'd' => num(t.day().into(), run),
        'H' => num(t.hour().into(), run),
        'h' => num(hour12.into(), run),
        'm' => num(t.minute().into(), run),
        's' => num(t.second().into(), run),
        'a' => if t.hour() < 12 { "AM" } else { "PM" }.into(),
        'E' => name(
            DAYS[usize::from(t.weekday().number_days_from_monday())],
            run,
        )
        .into(),
        _ => return None,
    })
}

/// A date pattern in the common `yyyy-MM-dd HH:mm:ss` notation; 'quoted' text is literal and other letters pass through.
pub fn format(t: OffsetDateTime, pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            let end = chars[i + 1..].iter().position(|&x| x == '\'');
            match end {
                Some(0) => out.push('\''),
                Some(n) => out.extend(&chars[i + 1..i + 1 + n]),
                None => out.extend(&chars[i + 1..]),
            }
            i += end.map_or(chars.len() - i, |n| n + 2);
        } else if c.is_ascii_alphabetic() {
            let run = chars[i..].iter().take_while(|&&x| x == c).count();
            match field(c, run, t) {
                Some(s) => out.push_str(&s),
                None => out.extend(std::iter::repeat_n(c, run)),
            }
            i += run;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}
