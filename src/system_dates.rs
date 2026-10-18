//! System locale, time zone and user-selected hour cycle, rather than a format
//! hard-coded to English/12-hour time. One formatter per rendering/worker thread.
use super::*;
use objc2::rc::Retained;
use objc2_foundation::{NSDate, NSDateFormatter, NSDateFormatterStyle, NSLocale, NSTimeZone};

struct Cached {
    formatter: Retained<NSDateFormatter>,
    date: Retained<NSDateFormatter>,
    time: Retained<NSDateFormatter>,
    refreshed: Instant,
}
thread_local! { static FORMATTER: RefCell<Option<Cached>> = const { RefCell::new(None) }; }

fn with_date<T>(time: Option<SystemTime>, f: impl FnOnce(&Cached, &NSDate) -> T) -> Option<T> {
    let time = time?;
    let seconds = match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    };
    FORMATTER.with(|cache| {
        let mut cache = cache.borrow_mut();
        // Also pick up preferences changed while Schnauz stays open. Formatting
        // visible rows reuses this cache; no per-file preference reads or I/O.
        if cache
            .as_ref()
            .is_none_or(|c| c.refreshed.elapsed() >= Duration::from_secs(1))
        {
            let make = |date, time| {
                let formatter = NSDateFormatter::new();
                formatter.setLocale(Some(&NSLocale::autoupdatingCurrentLocale()));
                formatter.setTimeZone(Some(&NSTimeZone::localTimeZone()));
                formatter.setDateStyle(date);
                formatter.setTimeStyle(time);
                formatter
            };
            let (no_style, short_style) = (
                NSDateFormatterStyle::NoStyle,
                NSDateFormatterStyle::ShortStyle,
            );
            let formatter = make(short_style, short_style);
            let date = make(short_style, no_style);
            let time = make(no_style, short_style);
            *cache = Some(Cached {
                formatter,
                date,
                time,
                refreshed: Instant::now(),
            });
        }
        Some(f(
            cache.as_ref().unwrap(),
            &NSDate::dateWithTimeIntervalSince1970(seconds),
        ))
    })
}

pub(super) fn format(time: Option<SystemTime>) -> String {
    with_date(time, |cache, date| {
        cache.formatter.stringFromDate(date).to_string()
    })
    .unwrap_or_else(|| "—".into())
}

/// Separate locale-aware fields; never split a combined date string on punctuation.
pub(super) fn parts(time: Option<SystemTime>) -> (String, String) {
    with_date(time, |cache, date| {
        (
            cache.date.stringFromDate(date).to_string(),
            cache.time.stringFromDate(date).to_string(),
        )
    })
    .unwrap_or_else(|| ("—".into(), "—".into()))
}
