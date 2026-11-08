use std::path::PathBuf;

use anyhow::{Context, Result, ensure};
use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

use crate::{
    browser::{BrowserSession, LOGIN_URL, preferred_school},
    config::Config,
    dates::{Month, SelectionRules, TaiwanHolidays, auto_select, submission_dates},
};

/// JSON request accepted by `nchu-diary-headless` on stdin.
///
/// Credentials are intentionally received for one run only. The headless binary never writes
/// this request to the config file and never includes the password in progress events.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct HeadlessRequest {
    pub job_id: Option<String>,
    pub id: String,
    #[serde(alias = "pw")]
    pub password: String,
    pub school: Option<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub target_days: Option<usize>,
    pub skip_weekends: Option<bool>,
    pub skip_holidays: Option<bool>,
    /// Optional explicit browser name: `Edge`, `Chrome`, or `Firefox`.
    pub browser: Option<String>,
    /// If omitted, writes to `reports/` below the current working directory.
    pub output_pdf: Option<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HeadlessEvent {
    Progress {
        job_id: Option<String>,
        message: String,
    },
    Done {
        job_id: Option<String>,
        school: String,
        year: i32,
        month: u32,
        submitted: usize,
        skipped: usize,
        pdf_path: PathBuf,
    },
    Error {
        job_id: Option<String>,
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct HeadlessResult {
    pub school: String,
    pub year: i32,
    pub month: u32,
    pub submitted: usize,
    pub skipped: usize,
    pub pdf_path: PathBuf,
}

struct RunOptions {
    requested_school: Option<String>,
    month: Month,
    rules: SelectionRules,
    holidays: TaiwanHolidays,
    output_pdf: Option<PathBuf>,
}

pub async fn run(request: HeadlessRequest, emit: impl Fn(HeadlessEvent)) -> Result<HeadlessResult> {
    ensure!(!request.id.trim().is_empty(), "登入帳號不可空白");
    ensure!(!request.password.is_empty(), "登入密碼不可空白");

    let today = Local::now().date_naive();
    let month = Month::new(
        request.year.unwrap_or(today.year()),
        request.month.unwrap_or(today.month()),
    )?;
    ensure!(month <= Month::of(today), "不可填寫或查詢今天之後的月份");
    let target_days = request.target_days.unwrap_or(15);
    ensure!(target_days <= 31, "target_days 必須介於 0 到 31 之間");
    let rules = SelectionRules {
        target_days,
        skip_weekends: request.skip_weekends.unwrap_or(true),
        skip_holidays: request.skip_holidays.unwrap_or(true),
    };
    let holidays = TaiwanHolidays::bundled()?;
    let config = Config {
        id: request.id,
        pw: request.password,
        ..Default::default()
    };
    let job_id = request.job_id.clone();
    let mut progress = |message: String| {
        emit(HeadlessEvent::Progress {
            job_id: job_id.clone(),
            message,
        });
    };

    let session = if let Some(browser) = request.browser.as_deref() {
        progress(format!("正在啟動 {browser} headless 瀏覽器…"));
        BrowserSession::launch_named(browser, true).await?
    } else {
        BrowserSession::launch_headless(&mut progress).await?
    };
    let options = RunOptions {
        requested_school: request.school,
        month,
        rules,
        holidays,
        output_pdf: request.output_pdf,
    };
    let result = run_with_session(&session, &config, options, &mut progress).await;
    session.close().await;
    result
}

async fn run_with_session(
    session: &BrowserSession,
    config: &Config,
    options: RunOptions,
    progress: &mut impl FnMut(String),
) -> Result<HeadlessResult> {
    progress("正在登入並讀取計畫單位…".into());
    let schools = session.login(config, LOGIN_URL).await?;
    let school_error = format!(
        "此帳號有多個計畫，請在 school 欄位指定校內編號；可用編號：{}",
        schools
            .iter()
            .map(|school| format!("{} ({})", school.value, school.label.trim()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let school = preferred_school(&schools, options.requested_school.as_deref(), false)
        .context(school_error)?;

    progress(format!("正在讀取計畫 {school} 的已有紀錄…"));
    let existing = session.prepare_school(&school).await?;
    let selected = auto_select(
        options.month,
        &options.rules,
        &existing,
        &options.holidays,
        Local::now().date_naive(),
    );
    let dates = submission_dates(
        options.month,
        &selected,
        &existing,
        Local::now().date_naive(),
    )?;
    let skipped = selected.len() - dates.len();
    progress(format!(
        "自動選取 {} 天，新增 {} 天…",
        selected.len(),
        dates.len()
    ));
    for (index, date) in dates.iter().enumerate() {
        progress(format!(
            "正在填寫 {date}（{} / {}）…",
            index + 1,
            dates.len()
        ));
        session.submit_date(&school, *date).await?;
    }

    progress("正在開啟學習日誌列印報表…".into());
    session
        .open_report(&school, options.month, Local::now().date_naive())
        .await?;
    let pdf_path = options
        .output_pdf
        .unwrap_or_else(|| default_pdf_path(&school, options.month));
    if let Some(parent) = pdf_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent).await?;
    }
    progress(format!("正在輸出 PDF：{}…", pdf_path.display()));
    session.save_current_pdf(&pdf_path).await?;

    Ok(HeadlessResult {
        school,
        year: options.month.year,
        month: options.month.month,
        submitted: dates.len(),
        skipped,
        pdf_path,
    })
}

fn default_pdf_path(school: &str, month: Month) -> PathBuf {
    let safe_school: String = school
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    PathBuf::from("reports").join(format!(
        "learning-log_{safe_school}_{}-{:02}.pdf",
        month.year, month.month
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_accepts_python_password_alias_and_defaults() {
        let request: HeadlessRequest = serde_json::from_str(
            r#"{"job_id":"job-1","id":"A123","pw":"secret","school":"115B1040"}"#,
        )
        .unwrap();
        assert_eq!(request.job_id.as_deref(), Some("job-1"));
        assert_eq!(request.password, "secret");
        assert_eq!(request.target_days, None);
        assert_eq!(request.skip_weekends, None);
    }

    #[test]
    fn event_protocol_is_tagged_json() {
        let event = HeadlessEvent::Progress {
            job_id: Some("job-1".into()),
            message: "正在登入…".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"progress""#));
        assert!(json.contains(r#""job_id":"job-1""#));
    }
}
