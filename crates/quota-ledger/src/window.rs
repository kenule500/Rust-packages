use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};

use crate::{Period, Window};

/// Computes the end of the window containing `now`.
pub(crate) fn reset_at(window: &Window, now: DateTime<Utc>) -> DateTime<Utc> {
    match window {
        Window::Rolling { duration } => now
            .checked_add_signed(*duration)
            .unwrap_or(DateTime::<Utc>::MAX_UTC),
        Window::Fixed { period, timezone } => {
            let local = now.with_timezone(timezone);
            let next = match period {
                Period::Daily => local
                    .date_naive()
                    .succ_opt()
                    .and_then(|date| date.and_hms_opt(0, 0, 0)),
                Period::Monthly => next_month(local.year(), local.month())
                    .and_then(|(year, month)| NaiveDate::from_ymd_opt(year, month, 1))
                    .and_then(|date| date.and_hms_opt(0, 0, 0)),
            };

            next.and_then(|naive| timezone.from_local_datetime(&naive).single())
                .map(|local| local.with_timezone(&Utc))
                .unwrap_or_else(|| now.checked_add_signed(Duration::days(1)).unwrap_or(now))
        }
    }
}

/// Computes the start of the window containing `now`.
pub(crate) fn start_at(window: &Window, now: DateTime<Utc>) -> DateTime<Utc> {
    match window {
        Window::Rolling { .. } => now,
        Window::Fixed { period, timezone } => {
            let local = now.with_timezone(timezone);
            let start = match period {
                Period::Daily => local.date_naive().and_hms_opt(0, 0, 0),
                Period::Monthly => NaiveDate::from_ymd_opt(local.year(), local.month(), 1)
                    .and_then(|date| date.and_hms_opt(0, 0, 0)),
            };

            start
                .and_then(|naive| timezone.from_local_datetime(&naive).single())
                .map(|local| local.with_timezone(&Utc))
                .unwrap_or(now)
        }
    }
}

fn next_month(year: i32, month: u32) -> Option<(i32, u32)> {
    match month {
        12 => year.checked_add(1).map(|year| (year, 1)),
        1..=11 => Some((year, month + 1)),
        _ => None,
    }
}
