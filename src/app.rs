use std::{fs, io::Write, os::unix::process::ExitStatusExt, process};

use anyhow::{Context, Result};
use colored::Colorize;
use keyring::Entry;
use nanoid::nanoid;
use serde::{Deserialize, Serialize};

use crate::libopenjudge::{self, Language, Problem, Submission, SubmissionResult};

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    user_email: Option<String>,
    last_problem: Option<String>,
}

impl AppConfig {
    fn read_config<P>(config_path: P) -> Result<Option<Self>>
    where
        P: AsRef<std::path::Path>,
    {
        let config = fs::read_to_string(config_path.as_ref())
            .map(Some)
            .or_else(|res| {
                if res.kind() == std::io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(res)
                }
            })?;
        match config {
            Some(config_str) => {
                let config: AppConfig = serde_json::from_str(&config_str)?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    fn write_config<P>(&self, config_path: P) -> Result<()>
    where
        P: AsRef<std::path::Path>,
    {
        let config = serde_json::to_string(self)?;
        fs::write(config_path, config)?;
        Ok(())
    }
}

fn print_problem(problem: &Problem) {
    println!("{}/{}\n", problem.group, problem.probset.bold());
    println!("{}\n", problem.title.on_yellow().bold());
    println!("{}\n", problem.description);
    if let Some(input) = &problem.input {
        println!("{}", "Input".yellow().bold());
        println!("{}\n", input);
    }
    if let Some(output) = &problem.output {
        println!("{}", "Output".yellow().bold());
        println!("{}\n", output);
    }
    if let Some(sample_input) = &problem.sample_input {
        println!("{}", "Sample Input".yellow().bold());
        println!("{}\n", sample_input);
    }
    if let Some(sample_output) = &problem.sample_output {
        println!("{}", "Sample Output".yellow().bold());
        println!("{}\n", sample_output);
    }
    if let Some(hint) = &problem.hint {
        println!("{}", "Hint".yellow().bold());
        println!("{}\n", hint);
    }
    if let Some(source) = &problem.source {
        println!("{}", "Source".yellow().bold());
        println!("{}\n", source);
    }
}

fn print_submission(submission: &Submission) {
    match &submission.result {
        SubmissionResult::Accepted => {
            println!("{}", "Accepted!".blue().bold());
        }
        SubmissionResult::CompileError { message } => {
            println!("{}", "Compile Error.".green().bold());
            println!("\n{}\n{}\n", "Compiler Diagnostics:".green(), message);
        }
        _ => {
            println!(
                "{}",
                match submission.result {
                    SubmissionResult::WrongAnswer => "Wrong Answer.",
                    SubmissionResult::TimeLimitExceeded => "Time Limit Exceeded.",
                    SubmissionResult::MemoryLimitExceeded => "Memory Limit Exceeded.",
                    SubmissionResult::RuntimeError => "Runtime Error.",
                    SubmissionResult::OutputLimitExceeded => "Output Limit Exceeded.",
                    SubmissionResult::PresentationError => "Presentation Error.",
                    _ => "Unknown error.",
                }
                .red()
                .bold()
            );
        }
    }
    println!("#{}", submission.id.white().bold());
    println!("Author:      {}", submission.author.white().bold());
    println!("Lang:        {}", submission.lang.white().bold());
    if let Some(time) = &submission.time {
        println!("Time:        {}", time.white().bold());
    }
    if let Some(memory) = &submission.memory {
        println!("Memory:      {}", memory.white().bold());
    }
    println!("Submit Time: {}", submission.submission_time.white().bold());
}

fn determine_language(file: &str, specified_lang: Option<String>) -> Result<Language> {
    let file = std::path::PathBuf::from(file);
    if !file.exists() {
        return Err(anyhow::anyhow!("File does not exist."))
            .context(format!("Reading {}", file.display()));
    }
    if !file.is_file() {
        return Err(
            anyhow::anyhow!("Path is not a file.").context(format!("Reading {}", file.display()))
        );
    }
    specified_lang.map(|lang| {
        match lang.to_lowercase()
            .as_str() {
                "c" | "gcc" => Ok(Language::Gcc),
                "cpp" | "g++" => Ok(Language::Gpp),
                "py" | "python" | "py3" | "python3" => Ok(Language::Python3),
                "pypy" | "pypy3" => Ok(Language::PyPy3),
                _ => Err(anyhow::anyhow!("Invalid language. Supported values: C, GCC, C++, G++, Py, Python, Py3, Python3, PyPy, PyPy3")).context(format!("Reading {}", file.display())),
            }
    })
    .unwrap_or_else(|| {
        match file
            .extension()
            .expect("Source code must provide an extension of '.c', '.cpp' or '.py', or specify the language with the --lang flag.")
            .to_str()
            .to_owned()
            .unwrap()
            .to_lowercase()
            .as_str()
        {
            "cpp" => Ok(Language::Gpp),
            "c" => Ok(Language::Gcc),
            "py" => Ok(Language::Python3),
            _ => Err(anyhow::anyhow!("Invalid file extension. Supported values: '.c', '.cpp', '.py', or specify the language with the --lang flag.").context(format!("Reading {}", file.display())))
        }
    })
}

fn get_config_dir() -> std::path::PathBuf {
    let config_root = dirs::home_dir().map_or_else(
        || std::env::current_dir().unwrap().join(".openjudge-cli"),
        |home| home.join(".openjudge-cli"),
    );
    if !config_root.exists() {
        fs::create_dir_all(&config_root).expect("Failed to create config directory.");
    }
    config_root.join("config.json")
}

pub async fn process_credentials(email: String) -> Result<()> {
    let password = rpassword::prompt_password("Enter your password: ")?;
    println!("Validating credentials with OpenJudge...");
    let client = libopenjudge::create_client().await?;
    libopenjudge::login(&client, &email, &password).await?;
    let config_old = AppConfig::read_config(get_config_dir())?;
    if let Some(ref config) = config_old {
        if let Some(ref user_email) = config.user_email {
            let entry = Entry::new("openjudge-cli", user_email)?;
            let _ = entry.delete_credential();
        }
    }
    let config = AppConfig {
        user_email: Some(email.clone()),
        ..config_old.unwrap_or_default()
    };
    let entry = Entry::new("openjudge-cli", &email)?;
    entry.set_password(&password)?;
    config.write_config(get_config_dir())?;
    println!("Credentials saved.");
    Ok(())
}

pub async fn view_problem(mut url: String) -> Result<()> {
    let mut config = AppConfig::read_config(get_config_dir())?.unwrap_or_default();
    if url == "." {
        url = config
            .last_problem
            .expect("No last problem found. Please specify a problem URL.");
    }
    let client = libopenjudge::create_client().await?;
    let problem = libopenjudge::get_problem(&client, &url).await?;
    print_problem(&problem);
    config.last_problem = Some(url);
    config.write_config(get_config_dir())?;
    Ok(())
}

async fn submit_solution_internal(
    url: &str,
    file: &str,
    lang: Language,
    email: &str,
    password: &str,
) -> Result<()> {
    let client = libopenjudge::create_client().await?;
    libopenjudge::login(&client, email, password).await?;
    let code = fs::read_to_string(file)?;
    println!("Submitting solution of {}", url.blue().underline());
    let submission_url = libopenjudge::submit_solution(&client, url, &code, lang).await?;
    println!(
        "Submission created at {}\nWaiting for judgement...",
        submission_url.blue().underline()
    );
    let submission = libopenjudge::query_submission_result(&client, &submission_url).await?;
    print_submission(&submission);
    Ok(())
}

pub async fn submit_solution(url: &str, file: &str, lang: Option<String>) -> Result<()> {
    let lang = determine_language(file, lang)?;
    let mut config = AppConfig::read_config(get_config_dir())?
        .expect("No user credentials found. Please run `openjudge-cli credentials` first.");
    let email = config
        .user_email
        .as_ref()
        .expect("No user credentials found. Please run `openjudge-cli credentials` first.");
    let entry = Entry::new("openjudge-cli", email)?;
    let password = entry
        .get_password()
        .expect("No user credentials found. Please run `openjudge-cli credentials` first.");
    let mut url = url.to_string();
    if url == "." {
        url = config
            .last_problem
            .expect("No last problem found. Please specify a problem URL.");
    }
    submit_solution_internal(&url, file, lang, email, &password).await?;
    config.last_problem = Some(url);
    config.write_config(get_config_dir())?;
    Ok(())
}

pub async fn test_solution(
    url: &str,
    file: &str,
    lang: Option<String>,
    submit: bool,
) -> Result<()> {
    let mut config = AppConfig::read_config(get_config_dir())?.unwrap_or_default();
    let mut url = url.to_string();
    if url == "." {
        url = config
            .last_problem
            .expect("No last problem found. Please specify a problem URL.");
    }
    let lang = determine_language(file, lang)?;
    let client = libopenjudge::create_client().await?;
    let problem = libopenjudge::get_problem(&client, &url).await?;
    if problem.sample_input.is_none() || problem.sample_output.is_none() {
        return Err(anyhow::anyhow!("No sample input/output found for problem."));
    }
    println!(
        "Testing solution {} of problem {}",
        file.blue().underline(),
        problem.title.blue().underline()
    );
    let mut input = problem.sample_input.unwrap_or_default();
    if input.as_str() == "(无)" || input.as_str() == "（无）" {
        input = "".to_string();
    }
    println!("{}", "Case Input:".yellow().bold());
    println!("{}", input);
    let output = problem.sample_output.unwrap_or_default();
    let code_output = match lang {
        Language::Gcc => {
            // .exe used for Windows compatibility
            let excutable_path = format!("./sol-{}.exe", nanoid!());
            process::Command::new("gcc")
                .arg("-o")
                .arg(&excutable_path)
                .arg(file)
                .spawn()?
                .wait()?;
            let mut child_process = process::Command::new(&excutable_path)
                .stdin(process::Stdio::piped())
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped())
                .spawn()?;
            child_process
                .stdin
                .take()
                .expect("Handle to stdin not available.")
                .write_all(input.as_bytes())?;
            let output = child_process.wait_with_output()?;
            let _ = fs::remove_file(&excutable_path);
            output
        }
        Language::Gpp => {
            let excutable_path = format!("./sol-{}.exe", nanoid!());
            process::Command::new("g++")
                .arg("-o")
                .arg(&excutable_path)
                .arg(file)
                .spawn()?
                .wait()?;
            let mut child_process = process::Command::new(&excutable_path)
                .stdin(process::Stdio::piped())
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped())
                .spawn()?;
            child_process
                .stdin
                .take()
                .expect("Handle to stdin not available.")
                .write_all(input.as_bytes())?;
            let output = child_process.wait_with_output()?;
            let _ = fs::remove_file(&excutable_path);
            output
        }
        Language::PyPy3 => {
            let mut child_process = process::Command::new("pypy3")
                .arg(file)
                .stdin(process::Stdio::piped())
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped())
                .spawn()?;
            child_process
                .stdin
                .take()
                .expect("Handle to stdin not available.")
                .write_all(input.as_bytes())?;
            child_process.wait_with_output()?
        }
        Language::Python3 => {
            let mut child_process = process::Command::new("python3")
                .arg(file)
                .stdin(process::Stdio::piped())
                .stdout(process::Stdio::piped())
                .stderr(process::Stdio::piped())
                .spawn()?;
            child_process
                .stdin
                .take()
                .expect("Handle to stdin not available.")
                .write_all(input.as_bytes())?;
            child_process.wait_with_output()?
        }
    };
    if code_output.status.success() {
        let code_output = String::from_utf8(code_output.stdout)?;
        if code_output.trim() == output.trim() {
            println!("{}", "Accepted!".blue().bold());
            if submit {
                submit_solution_internal(
                    &url,
                    file,
                    lang,
                    config.user_email.as_ref().expect(
                        "No user credentials found. Please run `openjudge-cli credentials` first.",
                    ),
                    &Entry::new("openjudge-cli", config.user_email.as_ref().unwrap())?
                        .get_password()?,
                )
                .await?;
            }
        } else {
            println!("{}", "Wrong Answer.".red().bold());
            println!("{}", "Expected Output:".yellow().bold());
            println!("{}", output);
            println!("{}", "Your Output:".yellow().bold());
            println!("{}", code_output);
        }
    } else {
        println!("{}", "Runtime Error.".red().bold());
        println!(
            "Exit Code: {}",
            code_output.status.code().unwrap_or_default()
        );
        println!(
            "Signal: {}",
            code_output.status.signal().unwrap_or_default()
        );
        println!("STDOUT:\n{}", String::from_utf8(code_output.stdout)?);
        println!("STDERR:\n{}", String::from_utf8(code_output.stderr)?);
    }
    config.last_problem = Some(url);
    config.write_config(get_config_dir())?;
    Ok(())
}

pub async fn search(group: &str, query: &str) -> Result<()> {
    println!("Searching for {} in group {}...", query.bold(), group.bold());
    let client = libopenjudge::create_client().await?;
    let result = libopenjudge::search(&client, &group, &query).await?;
    println!();
    println!("Found {} results:", result.len().to_string().bold());
    for item in &result {
        println!("#{} {} {}/{}", item.id, item.title.yellow().bold(), item.group, item.probset.bold());
        println!("{}", item.url.blue().underline().bold());
        println!("AC/Submissions: {}/{}", item.accepted_cnt.to_string().blue(), item.submission_cnt);
        println!();
    }

    Ok(())
}