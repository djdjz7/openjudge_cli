use anyhow::{Context, Result};
use colored::Colorize;
use keyring::Entry;
use nanoid::nanoid;
use onig::{self, Regex};
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::{fmt::Write as fmtWrite, fs, io::Write, process};
use syntect::{
    easy::HighlightLines, highlighting::Style, parsing::SyntaxSet, util::as_24_bit_terminal_escaped,
};

use crate::{
    code_theme,
    display::*,
    libopenjudge::{self, Language, Problem},
    utils::{
        html::{GraphicsProtocol, get_printable_html_text},
        interactions::{self, select_within},
    },
};

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    user_email: Option<String>,
    last_problem: Option<String>,
    graphics_protocol: Option<GraphicsProtocol>,
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

fn ensure_account(config: &Option<AppConfig>) -> Result<(&str, String)> {
    let email = config
        .as_ref()
        .and_then(|config| config.user_email.as_ref())
        .ok_or_else(|| anyhow::anyhow!(NO_CREDENTIALS_FOUND))?;
    let entry = Entry::new("openjudge-cli", email)?;
    let password = entry.get_password().expect(NO_CREDENTIALS_FOUND);
    Ok((email, password))
}

fn ensure_last_problem<'a>(specified: &'a str, config: &'a Option<AppConfig>) -> Result<&'a str> {
    if specified == "." {
        return match config {
            Some(config) => Ok(config
                .last_problem
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!(NO_LAST_PROBLEM_FOUND))?),
            None => Err(anyhow::anyhow!(NO_LAST_PROBLEM_FOUND)),
        };
    }
    Ok(specified)
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

pub async fn view_problem(url: &str) -> Result<()> {
    println!("Fetching problem details...");
    let config = AppConfig::read_config(get_config_dir())?;
    let url = ensure_last_problem(url, &config)?;
    let client = libopenjudge::create_client().await?;
    let problem = libopenjudge::get_problem(&client, url).await?;
    let graphics_protocol = config
        .as_ref()
        .map(|x| x.graphics_protocol.unwrap_or(GraphicsProtocol::Auto))
        .unwrap_or(GraphicsProtocol::Auto);
    macro_rules! map_optional_printable {
        ($field: expr) => {
            if let Some(s) = $field {
                Some(get_printable_html_text(s, graphics_protocol).await)
            } else {
                None
            }
        };
    }
    let problem_print = Problem {
        description: get_printable_html_text(&problem.description, graphics_protocol).await,
        input: map_optional_printable!(&problem.input),
        output: map_optional_printable!(&problem.output),
        sample_input: map_optional_printable!(&problem.sample_input),
        sample_output: map_optional_printable!(&problem.sample_output),
        hint: map_optional_printable!(&problem.hint),
        source: map_optional_printable!(&problem.source),
        ..problem
    };
    print!("{}", &problem_print);
    AppConfig {
        last_problem: Some(url.to_string()),
        ..config.unwrap_or_default()
    }
    .write_config(get_config_dir())?;
    Ok(())
}

async fn submit_solution_internal(
    urls: Vec<&str>,
    file: &str,
    lang: Language,
    email: &str,
    password: &str,
) -> Result<()> {
    let client = libopenjudge::create_client().await?;
    libopenjudge::login(&client, email, password).await?;
    let code = fs::read_to_string(file)?;
    for url in urls {
        println!("Submitting solution of {}", url.blue().underline());
        let submission_url = libopenjudge::submit_solution(&client, url, &code, lang).await?;
        println!(
            "Submission created at {}\nWaiting for judgement...",
            submission_url.blue().underline()
        );
        let submission = libopenjudge::query_submission_result(&client, &submission_url).await?;
        print!("{}", &submission);
    }
    Ok(())
}

pub async fn submit_solution(urls: Vec<&str>, file: &str, lang: Option<String>) -> Result<()> {
    let lang = determine_language(file, lang)?;
    let config = AppConfig::read_config(get_config_dir())?;
    let (email, password) = ensure_account(&config)?;
    if urls.len() == 1 {
        let url = urls[0];
        let url = ensure_last_problem(url, &config)?;
        submit_solution_internal(vec![url], file, lang, email, &password).await?;
        AppConfig {
            last_problem: Some(url.to_string()),
            ..config.unwrap_or_default()
        }
        .write_config(get_config_dir())?;
        Ok(())
    } else {
        let urls = urls
            .iter()
            .map(|url| -> Result<_> {
                let url = ensure_last_problem(url, &config)?;
                Ok(url)
            })
            .collect::<Result<Vec<_>>>()?;
        submit_solution_internal(urls, file, lang, email, &password).await?;
        Ok(())
    }
}

pub async fn test_solution(
    url: &str,
    file: &str,
    lang: Option<String>,
    submit: bool,
) -> Result<()> {
    let config = AppConfig::read_config(get_config_dir())?;
    let url = ensure_last_problem(url, &config)?;
    let lang = determine_language(file, lang)?;
    let client = libopenjudge::create_client().await?;
    let problem = libopenjudge::get_problem(&client, url).await?;
    if problem.sample_input.is_none() || problem.sample_output.is_none() {
        return Err(anyhow::anyhow!("No sample input/output found for problem."));
    }
    println!(
        "Testing solution {} of problem {}",
        file.blue().underline(),
        problem.title.blue().underline()
    );

    let mut input = if let Some(s) = &problem.sample_input {
        get_printable_html_text(s, GraphicsProtocol::Disabled).await
    } else {
        String::new()
    };
    let output = if let Some(s) = &problem.sample_output {
        get_printable_html_text(s, GraphicsProtocol::Disabled).await
    } else {
        String::new()
    };

    if input.as_str() == "(无)" || input.as_str() == "（无）" {
        input = "".to_string();
    }

    println!("{}", "Case Input:".yellow().bold());
    println!("{}", input);
    let code_output = match lang {
        Language::Gcc | Language::Gpp => {
            // .exe used for Windows compatibility
            let excutable_path = format!("./sol-{}.exe", nanoid!());
            process::Command::new(if lang == Language::Gcc { "gcc" } else { "g++" })
                .arg("--std=gnu++14")
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
        Language::PyPy3 | Language::Python3 => {
            let mut child_process = process::Command::new(if lang == Language::PyPy3 {
                "pypy3"
            } else {
                "python3"
            })
            .arg(file)
            .env("PYTHON_COLORS", "1")
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
                let (email, password) = ensure_account(&config)?;
                submit_solution_internal(vec![url], file, lang, email, &password).await?;
            }
        } else {
            let diff = TextDiff::from_lines(output.trim(), code_output.trim());
            println!("{}", "Wrong Answer.".red().bold());
            println!("{}", "Expected Output:".yellow().bold());
            println!("{}", output.trim());
            println!("{}", "Your Output:".yellow().bold());
            println!("{}", code_output.trim());
            println!("{}", "Diff:".yellow().bold());
            for change in diff.iter_all_changes() {
                let old_index = change
                    .old_index()
                    .map(|v| (v + 1).to_string())
                    .unwrap_or(" ".to_string());
                let new_index = change
                    .new_index()
                    .map(|v| (v + 1).to_string())
                    .unwrap_or(" ".to_string());
                match change.tag() {
                    ChangeTag::Delete => {
                        println!(
                            "{:>3} {:>3} | {} {}",
                            old_index,
                            new_index,
                            "-".red(),
                            change.value().trim().red()
                        );
                    }
                    ChangeTag::Insert => {
                        println!(
                            "{:>3} {:>3} | {} {}",
                            old_index,
                            new_index,
                            "+".green(),
                            change.value().trim().green()
                        );
                    }
                    ChangeTag::Equal => {
                        println!(
                            "{:>3} {:>3} |   {}",
                            old_index,
                            new_index,
                            change.value().trim()
                        );
                    }
                }
            }
        }
    } else {
        println!("{}", "Runtime Error.".red().bold());
        println!(
            "Exit Code: {}",
            code_output.status.code().unwrap_or_default()
        );
        #[cfg(unix)]
        {
            println!(
                "Signal: {}",
                code_output.status.signal().unwrap_or_default()
            );
        }
        println!("STDOUT:\n{}", String::from_utf8(code_output.stdout)?);
        println!("STDERR:\n{}", String::from_utf8(code_output.stderr)?);
    }
    AppConfig {
        last_problem: Some(url.to_string()),
        ..config.unwrap_or_default()
    }
    .write_config(get_config_dir())?;
    Ok(())
}

pub async fn search(group: &str, query: &str, interactive: bool) -> Result<()> {
    println!(
        "Searching for {} in group {}...",
        query.bold(),
        group.bold()
    );
    let client = libopenjudge::create_client().await?;
    let result = libopenjudge::search(&client, group, query).await?;
    println!();
    if !interactive {
        println!("Found {} results:", result.len().to_string().bold());
        for item in &result {
            println!("{}", item);
        }
        return Ok(());
    }
    let selected_index =
        interactions::select_within(&format!("Found {} results:", result.len()), &result, 4, 1);
    if let Some(index) = selected_index {
        let selected_problem = &result[index];
        view_problem(&selected_problem.url).await
    } else {
        println!("No problem selected.");
        Ok(())
    }
}

pub async fn view_user() -> Result<()> {
    println!("Fetching user details...");
    let config = AppConfig::read_config(get_config_dir())?;
    let (email, password) = ensure_account(&config)?;
    let client = libopenjudge::create_client().await?;
    libopenjudge::login(&client, email, &password).await?;
    let user = libopenjudge::get_user_info(&client).await?;
    print!("{}", user);
    Ok(())
}

pub async fn view_submission(url: &str) -> Result<()> {
    println!("Fetching submission details...");
    let config = AppConfig::read_config(get_config_dir())?;
    let (email, password) = ensure_account(&config)?;
    let client = libopenjudge::create_client().await?;
    libopenjudge::login(&client, email, &password).await?;
    let submission = libopenjudge::query_submission_result(&client, url).await?;
    println!("{}", submission);
    println!("{}", "Code".bold().on_white());
    let syntax_set = SyntaxSet::load_defaults_nonewlines();
    let syntax = syntax_set
        .find_syntax_by_extension(match submission.lang.as_str() {
            "Python3" => "py",
            "PyPy3" => "py",
            "G++" => "cpp",
            "GCC" => "c",
            _ => "text",
        })
        .unwrap();
    let mut highlighter = HighlightLines::new(syntax, &code_theme::ENKI_TOKYO_NIGHT_THEME);
    for line in submission.code.lines() {
        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(line, &syntax_set)?;
        let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
        println!("{}", escaped);
    }
    Ok(())
}

pub async fn list_submissions(problem_url: &str, interactive: bool) -> Result<()> {
    let config = AppConfig::read_config(get_config_dir())?;
    let problem_url = ensure_last_problem(problem_url, &config)?;
    let (email, password) = ensure_account(&config)?;
    let client = libopenjudge::create_client().await?;
    libopenjudge::login(&client, email, &password).await?;
    let submissions = libopenjudge::list_submissions(&client, problem_url).await?;

    if submissions.is_empty() {
        println!("{}", "No submissions found.".bold());
        return Ok(());
    }

    if !interactive {
        println!(
            "Found {} submissions:",
            submissions.len().to_string().bold()
        );
        for submission in &submissions {
            println!("{}", submission);
        }
        return Ok(());
    }
    let selected_index = select_within(
        &format!(
            "Found {} submissions:",
            submissions.len().to_string().bold(),
        ),
        &submissions,
        2,
        1,
    );
    match selected_index {
        None => Ok(()),
        Some(i) => {
            let selected_submission = &submissions[i];
            view_submission(&selected_submission.url).await
        }
    }
}

pub fn strip_slashes(text: &str) -> &str {
    let pattern = Regex::new(r#"^\/?(.*?)\/?$"#).unwrap();
    let captures = pattern.captures(text).unwrap();
    captures.at(1).unwrap_or("")
}

pub async fn list_probsets(group: &str, interactive: bool) -> Result<()> {
    println!("Fetching probsets...");
    let group_id = group;
    let client = libopenjudge::create_client().await?;
    let group = libopenjudge::get_group_info(&client, group).await?;
    if !interactive || group.probsets.is_empty() {
        println!("{}", group);
        return Ok(());
    }
    let mut prompt = String::new();
    writeln!(prompt, "{}", &group.name.bold())?;
    writeln!(prompt, "{}", &group.url.blue().underline())?;
    writeln!(prompt, "{}", &group.description)?;
    let selected_index = select_within(&prompt, &group.probsets, 2, 3);
    match selected_index {
        None => Ok(()),
        Some(i) => {
            let selected_probset = &group.probsets[i];
            list_problems(
                group_id,
                strip_slashes(&selected_probset.url),
                None,
                true,
                interactive,
            )
            .await
        }
    }
}

pub async fn list_problems(
    group: &str,
    probset: &str,
    page: Option<u32>,
    show_status: bool,
    interactive: bool,
) -> Result<()> {
    println!("Fetching problems...");
    let client = libopenjudge::create_client().await?;
    if show_status {
        let config = AppConfig::read_config(get_config_dir())?;
        let (email, password) = ensure_account(&config)?;
        libopenjudge::login(&client, email, &password).await?;
    }
    let problems = libopenjudge::get_partial_probset_info(&client, group, probset, page).await?;
    if !interactive {
        println!("{}", problems);
        return Ok(());
    }
    let mut prompt = String::new();
    writeln!(prompt, "{}/{}", problems.group_name, problems.name.bold())?;
    writeln!(prompt, "{}", problems.url.blue().underline())?;
    if problems.max_page != 1 {
        writeln!(
            prompt,
            "Displaying page {} of {}\n",
            problems.page.to_string().bold(),
            problems.max_page.to_string().bold()
        )?;
    };
    let mut options = problems
        .problems
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    if problems.page > 1 {
        options.push("Prev Page".to_owned());
    }
    if problems.page < problems.max_page {
        options.push("Next Page".to_owned());
    }
    let selected_index = select_within(&prompt, &options, 2, 3);
    match selected_index {
        None => Ok(()),
        Some(i) => match options[i].as_str() {
            "Next Page" => {
                Box::pin(list_problems(
                    group,
                    probset,
                    Some(problems.page + 1),
                    show_status,
                    interactive,
                ))
                .await
            }
            "Prev Page" => {
                Box::pin(list_problems(
                    group,
                    probset,
                    Some(problems.page - 1),
                    show_status,
                    interactive,
                ))
                .await
            }
            _ => {
                let rel = &problems.problems[i].url;
                let root = url::Url::parse(&format!("http://{}.openjudge.cn", group))?;
                let url = root.join(rel)?;
                Box::pin(view_problem(url.as_str())).await
            }
        },
    }
}

pub fn configure(graphics: &str) -> Result<()> {
    let conf = AppConfig::read_config(get_config_dir())?;
    AppConfig {
        graphics_protocol: Some(graphics.parse()?),
        ..conf.unwrap_or_default()
    }
    .write_config(get_config_dir())?;
    Ok(())
}
