//! System locale, time zone and user-selected hour cycle, rather than a format
//! hard-coded to English/12-hour time. One formatter per rendering/worker thread.
use super::*;
use objc2::rc::Retained;
use objc2_foundation::{NSDate, NSDateFormatter, NSDateFormatterStyle, NSLocale, NSTimeZone};

struct Cached {
    formatter: Retained<NSDateFormatter>,
    refreshed: Instant,
}
thread_local! { static FORMATTER: RefCell<Option<Cached>> = const { RefCell::new(None) }; }

pub(super) fn format(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "—".into();
    };
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
            let formatter = NSDateFormatter::new();
            formatter.setLocale(Some(&NSLocale::autoupdatingCurrentLocale()));
            formatter.setTimeZone(Some(&NSTimeZone::localTimeZone()));
            formatter.setDateStyle(NSDateFormatterStyle::ShortStyle);
            formatter.setTimeStyle(NSDateFormatterStyle::ShortStyle);
            *cache = Some(Cached {
                formatter,
                refreshed: Instant::now(),
            });
        }
        cache
            .as_ref()
            .unwrap()
            .formatter
            .stringFromDate(&NSDate::dateWithTimeIntervalSince1970(seconds))
            .to_string()
    })
}
