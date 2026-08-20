use dioxus::document;

pub fn format_date(secs: u64) -> String {
    // 1. Time of day
    // let seconds_in_day = secs % 86400;
    // let hours = seconds_in_day / 3600;
    // let minutes = (seconds_in_day % 3600) / 60;
    // let seconds = seconds_in_day % 60;

    // 2. Days since 1970-01-01
    let days = (secs / 86400) as i64;

    // 3. Gregorian date calculation
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    // format!("{day:02}/{month:02}/{year} à {hours:02}:{minutes:02}")
    format!("{day:02}/{month:02}/{year}")
}

pub async fn sleep_ms(millis: u32) {
    // HORRENDOUS call to JS for wait helper
    // TODO fix later
    let _ = document::eval(&format!(
        "await new Promise(resolve => setTimeout(resolve, {millis}));"
    ))
    .await;
}
