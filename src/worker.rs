use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use chrono::{Datelike, Local, NaiveDate};
use tokio::sync::{mpsc as async_mpsc, watch};

use crate::{
    browser::{BrowserSession, LOGIN_URL, School, preferred_school},
    config::Config,
    dates::{Month, submission_dates},
};

pub enum Command {
    Login {
        config: Config,
        force_manual: bool,
    },
    SelectSchool(String),
    Submit {
        month: Month,
        dates: BTreeSet<NaiveDate>,
    },
    CloseBrowser,
}

pub enum Event {
    Progress(String),
    ChooseSchool(Vec<School>),
    DatesReady {
        school: String,
        existing: BTreeSet<NaiveDate>,
    },
    ReportReady {
        submitted: usize,
        skipped: usize,
        pdf_path: Option<PathBuf>,
    },
    Failed(String),
    BrowserClosed,
    Stopped,
}

/// The GUI only exchanges owned values with this thread. All WebDriver calls stay off the GUI thread.
pub struct Worker {
    sender: async_mpsc::UnboundedSender<Command>,
    pub events: mpsc::Receiver<Event>,
    shutdown: watch::Sender<bool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(
        config_path: PathBuf,
        demo: bool,
        repaint: impl Fn() + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = async_mpsc::unbounded_channel();
        let (events_sender, events) = mpsc::channel();
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let thread = thread::Builder::new()
            .name("browser-worker".into())
            .spawn(move || {
                let emit = |event| {
                    let _ = events_sender.send(event);
                    repaint();
                };
                match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => {
                        runtime.block_on(run(receiver, shutdown_receiver, config_path, demo, &emit))
                    }
                    Err(error) => emit(Event::Failed(format!("無法啟動背景工作：{error}"))),
                }
                emit(Event::Stopped);
            })?;
        Ok(Self {
            sender,
            events,
            shutdown,
            thread: Some(thread),
        })
    }

    pub fn send(&self, command: Command) -> Result<()> {
        self.sender
            .send(command)
            .map_err(|_| anyhow::anyhow!("背景工作已結束，請重新啟動程式"))
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(thread) = self.thread.take().filter(|thread| thread.is_finished()) {
            let _ = thread.join();
        }
    }
}

struct State {
    session: Option<BrowserSession>,
    config: Config,
    school: Option<String>,
    existing: BTreeSet<NaiveDate>,
    report_open: bool,
    failed: bool,
    demo: bool,
    config_path: PathBuf,
}

fn report_pdf_path(config_path: &Path, school: &str, month: Month) -> PathBuf {
    let parent = config_path.parent().unwrap_or_else(|| Path::new("."));
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
    parent.join("reports").join(format!(
        "learning-log_{safe_school}_{}-{:02}.pdf",
        month.year, month.month
    ))
}

async fn run(
    mut receiver: async_mpsc::UnboundedReceiver<Command>,
    mut shutdown: watch::Receiver<bool>,
    config_path: PathBuf,
    demo: bool,
    emit: &impl Fn(Event),
) {
    let mut state = State {
        session: None,
        config: Config::default(),
        school: None,
        existing: BTreeSet::new(),
        report_open: false,
        failed: false,
        demo,
        config_path,
    };
    let mut ticker = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            command = receiver.recv() => {
                let Some(command) = command else { break };
                let result = tokio::select! {
                    biased;
                    _ = shutdown.changed() => break,
                    result = state.handle(command, emit) => result,
                };
                if let Err(error) = result {
                    state.failed = true;
                    emit(Event::Failed(format!("{error:#}")));
                }
            }
            _ = ticker.tick(), if state.session.is_some() => {
                if let Some(session) = &state.session
                    && !session.is_open().await {
                    state.close().await;
                    emit(Event::BrowserClosed);
                }
            }
        }
    }
    state.close().await;
}

impl State {
    async fn close(&mut self) {
        if let Some(session) = self.session.take() {
            session.close().await;
        }
        self.school = None;
        self.report_open = false;
        self.failed = false;
    }

    async fn choose(&mut self, school: String, emit: &impl Fn(Event)) -> Result<()> {
        emit(Event::Progress(format!(
            "正在讀取計畫 {school} 的已有紀錄…"
        )));
        self.existing = if self.demo {
            let today = Local::now().date_naive();
            BTreeSet::from([
                NaiveDate::from_ymd_opt(today.year(), today.month(), 2).context("日期無效")?
            ])
        } else {
            self.session
                .as_ref()
                .context("尚未登入")?
                .prepare_school(&school)
                .await?
        };
        self.config.school_value = Some(school.clone());
        if !self.demo {
            self.config.save(&self.config_path)?;
        }
        self.school = Some(school.clone());
        emit(Event::DatesReady {
            school,
            existing: self.existing.clone(),
        });
        Ok(())
    }

    async fn handle(&mut self, command: Command, emit: &impl Fn(Event)) -> Result<()> {
        match command {
            Command::Login {
                config,
                force_manual,
            } => {
                ensure!(self.session.is_none(), "請先關閉目前的瀏覽器");
                ensure!(
                    !config.id.trim().is_empty() && !config.pw.is_empty(),
                    "登入欄位不可空白"
                );
                self.config = config;
                self.failed = false;
                if !self.demo {
                    self.config.save(&self.config_path)?;
                }
                let schools = if self.demo {
                    vec![
                        School {
                            value: "DEMO-001".into(),
                            label: "DEMO-001 示範計畫一".into(),
                        },
                        School {
                            value: "DEMO-002".into(),
                            label: "DEMO-002 示範計畫二".into(),
                        },
                    ]
                } else {
                    self.session = Some(
                        BrowserSession::launch(|message| emit(Event::Progress(message))).await?,
                    );
                    emit(Event::Progress("正在登入並讀取計畫單位…".into()));
                    self.session
                        .as_ref()
                        .context("瀏覽器未啟動")?
                        .login(&self.config, LOGIN_URL)
                        .await?
                };
                if let Some(school) =
                    preferred_school(&schools, self.config.school_value.as_deref(), force_manual)
                {
                    self.choose(school, emit).await?;
                } else {
                    emit(Event::ChooseSchool(schools));
                }
            }
            Command::SelectSchool(school) => {
                ensure!(!self.failed && !self.report_open, "請重新登入");
                self.choose(school, emit).await?;
            }
            Command::Submit { month, dates } => {
                ensure!(
                    !self.failed && !self.report_open,
                    "本次工作已結束，請重新登入"
                );
                let school = self.school.as_deref().context("尚未選擇計畫")?;
                if !self.demo {
                    // Re-read immediately before submitting in case records changed while the calendar was open.
                    self.existing = self
                        .session
                        .as_ref()
                        .context("尚未登入")?
                        .prepare_school(school)
                        .await?;
                }
                let today = Local::now().date_naive();
                let dates_to_submit = submission_dates(month, &dates, &self.existing, today)?;
                let skipped = dates.len() - dates_to_submit.len();
                for (index, date) in dates_to_submit.iter().enumerate() {
                    emit(Event::Progress(format!(
                        "{}填寫 {date}（{} / {}）",
                        if self.demo { "示範：" } else { "" },
                        index + 1,
                        dates_to_submit.len()
                    )));
                    if !self.demo {
                        self.session
                            .as_ref()
                            .context("尚未登入")?
                            .submit_date(school, *date)
                            .await?;
                    }
                    self.existing.insert(*date);
                }
                emit(Event::Progress("正在設定報表查詢區間…".into()));
                if !self.demo {
                    self.session
                        .as_ref()
                        .context("尚未登入")?
                        .open_report(school, month, Local::now().date_naive())
                        .await?;
                }
                let pdf_path = if self.demo {
                    None
                } else {
                    let path = report_pdf_path(&self.config_path, school, month);
                    if let Some(parent) = path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    emit(Event::Progress(format!(
                        "正在下載 PDF：{}…",
                        path.display()
                    )));
                    self.session
                        .as_ref()
                        .context("尚未登入")?
                        .save_current_pdf(&path)
                        .await?;
                    Some(path)
                };
                self.report_open = true;
                emit(Event::ReportReady {
                    submitted: dates_to_submit.len(),
                    skipped,
                    pdf_path,
                });
            }
            Command::CloseBrowser => {
                self.close().await;
                emit(Event::BrowserClosed);
            }
        }
        Ok(())
    }
}
