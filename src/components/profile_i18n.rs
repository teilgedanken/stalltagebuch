use crate::models::Gender;
use chrono::{Datelike, NaiveDate};
use dioxus_i18n::tid;

pub fn gender_label(gender: &Gender) -> String {
    match gender {
        Gender::Male => tid!("gender-male"),
        Gender::Female => tid!("gender-female"),
        Gender::Unknown => tid!("gender-unknown"),
    }
}

pub fn format_age_years_months(birth_date: NaiveDate, today: NaiveDate) -> String {
    if birth_date > today {
        return format!("0 {}", tid!("period-months"));
    }

    let mut total_months = (today.year() - birth_date.year()) * 12
        + (today.month() as i32 - birth_date.month() as i32);

    if today.day() < birth_date.day() {
        total_months -= 1;
    }

    if total_months < 0 {
        total_months = 0;
    }

    let years = total_months / 12;
    let months = total_months % 12;

    if years > 0 {
        let years_label = if years == 1 {
            tid!("period-year")
        } else {
            tid!("period-years")
        };

        if months > 0 {
            let months_label = if months == 1 {
                tid!("period-month")
            } else {
                tid!("period-months")
            };
            format!("{} {} {} {}", years, years_label, months, months_label)
        } else {
            format!("{} {}", years, years_label)
        }
    } else {
        let months_label = if months == 1 {
            tid!("period-month")
        } else {
            tid!("period-months")
        };
        format!("{} {}", months, months_label)
    }
}
