use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

pub const WORK_CONTENT: &str = "協助教授計畫";
pub const MIN_YEAR: i32 = 1912;
pub const MAX_YEAR: i32 = 2099;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Month {
    pub year: i32,
    pub month: u32,
}

impl Month {
    pub fn new(year: i32, month: u32) -> Result<Self> {
        ensure!(
            (MIN_YEAR..=MAX_YEAR).contains(&year),
            "年份須介於 {MIN_YEAR} 與 {MAX_YEAR}"
        );
        ensure!((1..=12).contains(&month), "月份須介於 1 與 12");
        Ok(Self { year, month })
    }

    pub fn of(date: NaiveDate) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
        }
    }

    pub fn days(self) -> impl Iterator<Item = NaiveDate> {
        (1..=31).filter_map(move |day| NaiveDate::from_ymd_opt(self.year, self.month, day))
    }

    pub fn contains(self, date: NaiveDate) -> bool {
        self == Self::of(date)
    }

    pub fn shift(self, offset: i32) -> Self {
        let index = self.year * 12 + self.month as i32 - 1 + offset;
        Self::new(index.div_euclid(12), index.rem_euclid(12) as u32 + 1).unwrap_or(self)
    }

    pub fn report_range(self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        let start = NaiveDate::from_ymd_opt(self.year, self.month, 1).expect("validated month");
        let end = if self.contains(today) {
            today
        } else {
            self.days().last().expect("validated month")
        };
        (start, end)
    }
}

pub fn roc_date(date: NaiveDate) -> String {
    format!(
        "{:03}{:02}{:02}",
        date.year() - 1911,
        date.month(),
        date.day()
    )
}

pub fn parse_roc(value: &str) -> Option<NaiveDate> {
    let value = value.trim();
    if value.len() != 7 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    NaiveDate::from_ymd_opt(
        value[..3].parse::<i32>().ok()? + 1911,
        value[3..5].parse().ok()?,
        value[5..].parse().ok()?,
    )
}

#[derive(Deserialize)]
pub struct TaiwanHolidays {
    pub source: String,
    pub start_year: i32,
    pub end_year: i32,
    dates: BTreeMap<NaiveDate, String>,
}

impl TaiwanHolidays {
    pub fn bundled() -> Result<Self> {
        serde_json::from_str(include_str!("../data/taiwan_holidays.json"))
            .context("內建臺灣假日資料無法讀取")
    }

    pub fn name(&self, date: NaiveDate) -> Option<&str> {
        self.dates.get(&date).map(String::as_str)
    }

    pub fn contains(&self, date: NaiveDate) -> bool {
        self.dates.contains_key(&date)
    }
}

#[derive(Clone)]
pub struct SelectionRules {
    pub target_days: usize,
    pub skip_weekends: bool,
    pub skip_holidays: bool,
}

impl Default for SelectionRules {
    fn default() -> Self {
        Self {
            target_days: 15,
            skip_weekends: true,
            skip_holidays: true,
        }
    }
}

/// Like the Python version, eligible existing logs count toward N and are never resubmitted.
/// Selection covers the entire month, including future dates, to preserve existing behavior.
pub fn auto_select(
    month: Month,
    rules: &SelectionRules,
    existing: &BTreeSet<NaiveDate>,
    holidays: &TaiwanHolidays,
) -> BTreeSet<NaiveDate> {
    month
        .days()
        .filter(|date| !rules.skip_weekends || date.weekday().number_from_monday() <= 5)
        .filter(|date| !rules.skip_holidays || !holidays.contains(*date))
        .take(rules.target_days)
        .filter(|date| !existing.contains(date))
        .collect()
}

pub fn submission_dates(
    month: Month,
    dates: &BTreeSet<NaiveDate>,
    existing: &BTreeSet<NaiveDate>,
) -> Result<Vec<NaiveDate>> {
    ensure!(
        dates.iter().all(|date| month.contains(*date)),
        "選取日期必須在報表月份內"
    );
    Ok(dates.difference(existing).copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(value: &str) -> NaiveDate {
        value.parse().unwrap()
    }

    #[test]
    fn roc_conversion_and_validation() {
        for (iso, roc) in [
            ("2024-03-05", "1130305"),
            ("2026-01-09", "1150109"),
            ("1912-01-01", "0010101"),
        ] {
            assert_eq!(roc_date(d(iso)), roc);
            assert_eq!(parse_roc(roc), Some(d(iso)));
        }
        for invalid in [
            "1150230",
            "1151301",
            "2026-01-01",
            "身分證",
            "１１５０１０１",
        ] {
            assert_eq!(parse_roc(invalid), None);
        }
    }

    #[test]
    fn existing_eligible_dates_count_towards_target() -> Result<()> {
        let holidays = TaiwanHolidays::bundled()?;
        let existing = BTreeSet::from([d("2026-01-02"), d("2026-01-03")]);
        let rules = SelectionRules {
            target_days: 3,
            ..Default::default()
        };
        assert_eq!(
            auto_select(Month::new(2026, 1)?, &rules, &existing, &holidays),
            BTreeSet::from([d("2026-01-05"), d("2026-01-06")])
        );
        Ok(())
    }

    #[test]
    fn holiday_and_weekend_filters_can_be_disabled() -> Result<()> {
        let month = Month::new(2026, 1)?;
        let holidays = TaiwanHolidays::bundled()?;
        let rules = SelectionRules {
            target_days: 3,
            skip_weekends: false,
            skip_holidays: false,
        };
        assert_eq!(
            auto_select(month, &rules, &BTreeSet::new(), &holidays),
            BTreeSet::from([d("2026-01-01"), d("2026-01-02"), d("2026-01-03")])
        );
        assert!(
            auto_select(
                month,
                &SelectionRules {
                    target_days: 0,
                    ..rules
                },
                &BTreeSet::new(),
                &holidays
            )
            .is_empty()
        );
        Ok(())
    }

    #[test]
    fn lunar_holidays_work_across_year_changes() -> Result<()> {
        let holidays = TaiwanHolidays::bundled()?;
        assert!(holidays.contains(d("2026-02-17")));
        assert!(holidays.contains(d("2027-02-06")));
        assert!(!holidays.contains(d("2026-02-20")));
        Ok(())
    }

    #[test]
    fn report_ranges_handle_today_and_leap_years() -> Result<()> {
        let today = d("2026-11-08");
        assert_eq!(
            Month::new(2026, 11)?.report_range(today),
            (d("2026-11-01"), today)
        );
        assert_eq!(
            Month::new(2024, 2)?.report_range(today),
            (d("2024-02-01"), d("2024-02-29"))
        );
        assert_eq!(Month::new(2026, 12)?.shift(1), Month::new(2027, 1)?);
        assert_eq!(Month::new(MIN_YEAR, 1)?.shift(-1), Month::new(MIN_YEAR, 1)?);
        Ok(())
    }

    #[test]
    fn submission_rechecks_duplicates_and_rejects_mixed_months() -> Result<()> {
        let month = Month::new(2026, 11)?;
        let selected = BTreeSet::from([d("2026-11-03"), d("2026-11-02")]);
        assert_eq!(
            submission_dates(month, &selected, &BTreeSet::from([d("2026-11-02")]))?,
            vec![d("2026-11-03")]
        );
        assert!(
            submission_dates(month, &BTreeSet::from([d("2026-12-01")]), &BTreeSet::new()).is_err()
        );
        Ok(())
    }
}
