//! Opt-in, real-browser integration test. All pages and submitted records are local fixtures.
use std::{
    collections::BTreeSet,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::Result;
use chrono::NaiveDate;
use nchu_diary_autofill::{
    browser::BrowserSession,
    config::Config,
    dates::{Month, WORK_CONTENT, submission_dates},
};

struct Fixture {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Fixture {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let url = format!("http://{}", listener.local_addr()?);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                if let Ok((mut stream, _)) = listener.accept() {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut reader = BufReader::new(&stream);
                    let mut first = String::new();
                    let _ = reader.read_line(&mut first);
                    let path = first
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("/")
                        .split('?')
                        .next()
                        .unwrap_or("/");
                    let body = match path {
                        "/" => include_str!("fixtures/login.html"),
                        "/home" => include_str!("fixtures/home.html"),
                        "/logs" => include_str!("fixtures/logs.html"),
                        "/report" => include_str!("fixtures/report.html"),
                        "/printed" => {
                            "<!doctype html><title>Printed</title><p id='printed'>Report ready</p>"
                        }
                        _ => "",
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                } else {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        });
        Ok(Self {
            url,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[tokio::test]
#[ignore = "Downloads a matching WebDriver and starts a headless browser; use --ignored to run"]
async fn login_switch_school_submit_and_print_without_touching_nchu() -> Result<()> {
    let fixture = Fixture::start()?;
    let name = std::env::var("NCHU_TEST_BROWSER").unwrap_or_else(|_| "Edge".into());
    let session = BrowserSession::launch_named(&name, true).await?;
    let result = async {
        let config = Config {
            id: "fixture-id".into(),
            pw: "fixture-pw".into(),
            ..Default::default()
        };
        let options = session.login(&config, &fixture.url).await?;
        assert_eq!(options.len(), 2);
        // Dates must be read from B after switching away from the default plan A.
        let existing = session.prepare_school("B").await?;
        let d = |value: &str| value.parse::<NaiveDate>().unwrap();
        assert_eq!(existing, BTreeSet::from([d("2026-11-03")]));
        let selected = BTreeSet::from([d("2026-11-03"), d("2026-11-02"), d("2026-11-04")]);
        let dates = submission_dates(Month::new(2026, 11)?, &selected, &existing)?;
        for date in dates {
            session.submit_date(date).await?;
        }
        let records: Vec<serde_json::Value> = session
            .driver
            .execute(
                "return JSON.parse(sessionStorage.getItem('logs:B'))",
                Vec::new(),
            )
            .await?
            .convert()?;
        assert_eq!(records.len(), 3);
        assert_eq!(records[1]["date"], "1151102");
        assert_eq!(records[1]["work"], WORK_CONTENT);
        assert_eq!(records[2]["date"], "1151104");
        let today = d("2026-11-08");
        for (month, start, end) in [
            (Month::new(2024, 2)?, "1130201", "1130229"),
            (Month::new(2026, 11)?, "1151101", "1151108"),
        ] {
            session.open_report("B", month, today).await?;
            let report: serde_json::Value = session
                .driver
                .execute(
                    "return JSON.parse(sessionStorage.getItem('report'))",
                    Vec::new(),
                )
                .await?
                .convert()?;
            assert_eq!(report["school"], "B");
            assert_eq!(report["start"], start);
            assert_eq!(report["end"], end);
        }
        anyhow::Ok(())
    }
    .await;
    session.close().await;
    result
}
