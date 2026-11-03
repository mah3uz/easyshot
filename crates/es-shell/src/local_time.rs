use chrono::{DateTime, TimeZone};
use std::fmt::Display;
use time::OffsetDateTime;

pub(crate) fn local_clock(t: OffsetDateTime, seconds: bool) -> String {
    clock_in(t, &chrono::Local, seconds)
}

fn clock_in<Tz: TimeZone>(t: OffsetDateTime, zone: &Tz, seconds: bool) -> String
where
    Tz::Offset: Display,
{
    match DateTime::from_timestamp(t.unix_timestamp(), 0) {
        Some(utc) => utc
            .with_timezone(zone)
            .format(if seconds { "%H:%M:%S" } else { "%H:%M" })
            .to_string(),
        None => {
            let utc = t.to_offset(time::UtcOffset::UTC);
            let secs = if seconds {
                format!(":{:02}", utc.second())
            } else {
                String::new()
            };
            format!("{:02}:{:02}{secs} UTC", utc.hour(), utc.minute())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn noon() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_793_793_600).unwrap()
    }

    #[test]
    fn the_clock_shows_the_zone_it_is_given_not_utc() {
        let sydney = FixedOffset::east_opt(11 * 3600).unwrap();
        assert_eq!(clock_in(noon(), &sydney, false), "23:00");
        assert_eq!(clock_in(noon(), &sydney, true), "23:00:00");
        let new_york = FixedOffset::west_opt(5 * 3600).unwrap();
        assert_eq!(clock_in(noon(), &new_york, false), "07:00");
    }
}
