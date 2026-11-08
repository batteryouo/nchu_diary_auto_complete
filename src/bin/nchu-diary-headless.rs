use std::{
    io::{self, Read, Write},
    process::ExitCode,
};

use nchu_diary_autofill::headless::{HeadlessEvent, HeadlessRequest, run};

fn write_event(event: HeadlessEvent) {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if let Ok(json) = serde_json::to_string(&event) {
        let _ = writeln!(stdout, "{json}");
        let _ = stdout.flush();
    }
}

fn print_help() {
    eprintln!(
        "NCHU Diary AutoFill headless API\n\n用法：將單一 JSON request 從 stdin 傳入，stdout 會輸出 JSON Lines 事件。\n\n範例：\n  Get-Content request.json -Raw | nchu-diary-headless.exe\n\nrequest 欄位：id、password（或 pw）、school、year、month、target_days、\nskip_weekends、skip_holidays、browser、output_pdf。"
    );
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        print_help();
        return ExitCode::SUCCESS;
    }

    let mut request_json = String::new();
    if let Err(error) = io::stdin().lock().read_to_string(&mut request_json) {
        write_event(HeadlessEvent::Error {
            job_id: None,
            code: "INPUT_ERROR".into(),
            message: error.to_string(),
        });
        return ExitCode::from(2);
    }
    let request: HeadlessRequest = match serde_json::from_str(&request_json) {
        Ok(request) => request,
        Err(error) => {
            write_event(HeadlessEvent::Error {
                job_id: None,
                code: "INVALID_REQUEST".into(),
                message: error.to_string(),
            });
            return ExitCode::from(2);
        }
    };
    let job_id = request.job_id.clone();
    match run(request, write_event).await {
        Ok(result) => {
            write_event(HeadlessEvent::Done {
                job_id,
                school: result.school,
                year: result.year,
                month: result.month,
                submitted: result.submitted,
                skipped: result.skipped,
                pdf_path: result.pdf_path,
            });
            ExitCode::SUCCESS
        }
        Err(error) => {
            write_event(HeadlessEvent::Error {
                job_id,
                code: "HEADLESS_ERROR".into(),
                message: format!("{error:#}"),
            });
            ExitCode::from(1)
        }
    }
}
