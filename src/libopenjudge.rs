use anyhow::Result;
use base64::prelude::*;
use reqwest::Client;
use scraper::{self, ElementRef};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Problem {
    pub title: String,
    pub group: String,
    pub probset: String,
    pub description: String,
    pub input: Option<String>,
    pub output: Option<String>,
    pub sample_input: Option<String>,
    pub sample_output: Option<String>,
    pub hint: Option<String>,
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    pub result: String,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SubmitResponse {
    pub result: String,
    pub message: Option<String>,
    pub redirect: Option<String>,
}

pub enum SubmissionResult {
    PresentationError,
    Accepted,
    CompileError { message: String },
    WrongAnswer,
    RuntimeError,
    TimeLimitExceeded,
    OutputLimitExceeded,
    MemoryLimitExceeded,
    Waiting,
    SystemError,
    Unknown,
}

pub enum Language {
    Gcc,
    Gpp,
    Python3,
    PyPy3,
}

impl From<Language> for &'static str {
    fn from(val: Language) -> &'static str {
        match val {
            Language::Gcc => "GCC",
            Language::Gpp => "G++",
            Language::Python3 => "Python3",
            Language::PyPy3 => "PyPy3",
        }
    }
}

pub struct Submission {
    pub result: SubmissionResult,
    pub id: String,
    pub author: String,
    pub lang: String,
    pub submission_time: String,
    pub memory: Option<String>,
    pub time: Option<String>,
}

pub struct ProblemSearchResult {
    pub title: String,
    pub url: String,
    pub group: String,
    pub probset: String,
    pub id: String,
    pub accepted_cnt: u32,
    pub submission_cnt: u32,
}

macro_rules! selector {
    ($selector: expr) => {
        scraper::Selector::parse($selector).unwrap()
    };
}

pub async fn create_client() -> Result<Client> {
    let client = Client::builder().cookie_store(true).build().unwrap();
    // we do this so that following requests will have the cookies
    client.get("http://openjudge.cn/").send().await?;
    Ok(client)
}

fn query_selector_inner_text(dom: &scraper::Html, selector: &scraper::Selector) -> String {
    let selector_target = dom.select(selector).next();
    if let Some(selector_target) = selector_target {
        let text = selector_target
            .text()
            .collect::<Vec<&str>>()
            .join("\n")
            .trim()
            .to_string();
        text
    } else {
        String::new()
    }
}

pub async fn get_problem(http_client: &Client, url: &str) -> Result<Problem> {
    let res = http_client.get(url).send().await?;
    let group_name_selector = selector!("#header .wrapper .contest-title-tab h2 a");
    let prob_set_selector = selector!("#header .wrapper .contest-title-tab h2:nth-child(3)");
    let prob_title_selector = selector!("#pageTitle h2");
    let prob_content_dt_selector = selector!(".problem-content dt");

    let body = res.text().await?;
    let dom = scraper::Html::parse_document(&body);
    let group = query_selector_inner_text(&dom, &group_name_selector);
    let probset = query_selector_inner_text(&dom, &prob_set_selector);
    let problem_content_dts = dom.select(&prob_content_dt_selector).collect::<Vec<_>>();
    let title = query_selector_inner_text(&dom, &prob_title_selector);
    let mut description = String::new();
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut sample_input: Option<String> = None;
    let mut sample_output: Option<String> = None;
    let mut hint: Option<String> = None;
    let mut source: Option<String> = None;
    for dt in problem_content_dts {
        let dt_text = dt.text().collect::<Vec<&str>>().join("\n");
        let dd = dt
            .next_siblings()
            .find(|element| element.value().is_element());
        if let Some(dd) = dd {
            let dd_text = ElementRef::wrap(dd)
                .unwrap()
                .text()
                .collect::<Vec<&str>>()
                .join("\n")
                .trim()
                .to_string();
            match dt_text.as_str() {
                "描述" => description = dd_text,
                "输入" => input = Some(dd_text),
                "输出" => output = Some(dd_text),
                "样例输入" => sample_input = Some(dd_text),
                "样例输出" => sample_output = Some(dd_text),
                "提示" => hint = Some(dd_text),
                "来源" => source = Some(dd_text),
                _ => {}
            }
        }
    }

    Ok(Problem {
        title,
        group,
        probset,
        description,
        input,
        output,
        sample_input,
        sample_output,
        hint,
        source,
    })
}

pub async fn login(http_client: &Client, email: &str, password: &str) -> Result<()> {
    let response = http_client
        .post("http://openjudge.cn/api/auth/login")
        .form(&[("email", email), ("password", password)])
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Login Failed: {}",
            response.status().to_string()
        ));
    }
    let response_text = response.text().await?;
    let response: LoginResponse = serde_json::from_str(&response_text)?;
    if response.result != "SUCCESS" {
        return Err(anyhow::anyhow!(
            response
                .message
                .unwrap_or_else(|| "No message provided".to_string())
        ));
    }
    Ok(())
}

pub async fn submit_solution(
    http_client: &Client,
    url: &str,
    code: &str,
    lang: Language,
) -> Result<String> {
    let contest_id_selector = scraper::Selector::parse(r#"input[name="contestId"]"#).unwrap();
    let problem_number_selector =
        scraper::Selector::parse(r#"input[name="problemNumber"]"#).unwrap();
    let url = if url.ends_with("/") {
        format!("{}submit/", url)
    } else {
        format!("{}/submit/", url)
    };
    let submit_page = http_client.get(&url).send().await?.text().await?;
    let dom = scraper::Html::parse_document(&submit_page);
    let contest_id = dom
        .select(&contest_id_selector)
        .next()
        .unwrap()
        .value()
        .attr("value")
        .unwrap();
    let problem_number = dom
        .select(&problem_number_selector)
        .next()
        .unwrap()
        .value()
        .attr("value")
        .unwrap();
    let code = BASE64_STANDARD.encode(code);
    let url = url::Url::parse(url.as_str())?;
    let submit_api = format!("http://{}/api/solution/submitv2/", url.host_str().unwrap());
    let response = http_client
        .post(&submit_api)
        .form(&[
            ("contestId", contest_id),
            ("problemNumber", problem_number),
            ("sourceEncode", "base64"),
            ("language", lang.into()),
            ("source", &code),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Submission failed: {}",
            response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to get response text".to_string())
        ));
    }

    let response_text = response.text().await?;
    let response: SubmitResponse = serde_json::from_str(&response_text)?;
    if response.result != "SUCCESS" {
        return Err(anyhow::anyhow!(
            response
                .message
                .unwrap_or_else(|| "No message provided".to_string())
        ));
    }
    if response.redirect.is_none() {
        return Err(anyhow::anyhow!("No redirect URL provided."));
    }
    let redirect_url = response.redirect.unwrap();
    Ok(redirect_url)
}

pub async fn query_submission_result(
    http_client: &Client,
    result_page_url: &str,
) -> Result<Submission> {
    let compile_status_selector = selector!(".compile-status a");
    let compiler_info_selector = selector!(".submitStatus pre");
    let submission_details_dts_selector = selector!(".compile-info dt");
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    // this finishes instantly
    interval.tick().await;
    loop {
        let response = http_client.get(result_page_url).send().await?;
        let html = response.text().await?;
        let dom = scraper::Html::parse_document(&html);
        let status = query_selector_inner_text(&dom, &compile_status_selector);
        if status == "Waiting" {
            interval.tick().await;
        } else {
            let result = match status.as_str() {
                "Accepted" => SubmissionResult::Accepted,
                "Compile Error" => {
                    let message = query_selector_inner_text(&dom, &compiler_info_selector);
                    SubmissionResult::CompileError { message }
                }
                "Presentation Error" => SubmissionResult::PresentationError,
                "Wrong Answer" => SubmissionResult::WrongAnswer,
                "Runtime Error" => SubmissionResult::RuntimeError,
                "Time Limit Exceeded" => SubmissionResult::TimeLimitExceeded,
                "Output Limit Exceeded" => SubmissionResult::OutputLimitExceeded,
                "Memory Limit Exceeded" => SubmissionResult::MemoryLimitExceeded,
                "Waiting" => SubmissionResult::Waiting,
                "System Error" => SubmissionResult::SystemError,
                _ => SubmissionResult::Unknown,
            };
            let mut id = String::new();
            let mut author = String::new();
            let mut lang = String::new();
            let mut submission_time = String::new();
            let mut memory: Option<String> = None;
            let mut time: Option<String> = None;
            let submission_details_dts = dom
                .select(&submission_details_dts_selector)
                .collect::<Vec<_>>();
            for dt in submission_details_dts {
                let dt_text = dt.text().collect::<Vec<&str>>().join("\n");
                let dd = dt
                    .next_siblings()
                    .find(|element| element.value().is_element());
                if let Some(dd) = dd {
                    let dd_text = ElementRef::wrap(dd)
                        .unwrap()
                        .text()
                        .collect::<Vec<&str>>()
                        .join("\n");
                    match dt_text.as_str() {
                        "#:" => id = dd_text,
                        "提交人:" => author = dd_text,
                        "语言:" => lang = dd_text,
                        "提交时间:" => submission_time = dd_text,
                        "内存:" => memory = Some(dd_text),
                        "时间:" => time = Some(dd_text),
                        _ => {}
                    }
                }
            }

            return Ok(Submission {
                result,
                id,
                author,
                lang,
                submission_time,
                memory,
                time,
            });
        }
    }
}

pub async fn search(
    http_client: &Client,
    group: &str,
    query: &str,
) -> Result<Vec<ProblemSearchResult>> {
    let search_result_tr_selector = selector!("#main .problems-list tbody tr");
    let title_selector = selector!(".problem-title a");
    let id_selector = selector!(".problem-number");
    let accepted_cnt_selector = selector!(".accepted");
    let submit_cnt_selector = selector!(".submissions");
    let group_selector = selector!(".source a:nth-of-type(1)");
    let probset_selector = selector!(".source a:nth-of-type(2)");
    let url = format!("http://{}.openjudge.cn/search/?q={}", group, query);
    let response = http_client.get(&url).send().await?;
    let body = response.text().await?;
    let dom = scraper::Html::parse_document(&body);
    let mut results = Vec::new();
    for element in dom.select(&search_result_tr_selector) {
        let title_anchor = element.select(&title_selector).next().unwrap();
        let title = title_anchor.inner_html();
        let url = title_anchor.value().attr("href").unwrap().to_string();
        let id = element.select(&id_selector).next().unwrap().inner_html();
        let accepted_cnt = element
            .select(&accepted_cnt_selector)
            .next()
            .unwrap()
            .inner_html()
            .parse()?;
        let submit_cnt = element
            .select(&submit_cnt_selector)
            .next()
            .unwrap()
            .inner_html()
            .parse()?;
        let group = element.select(&group_selector).next().unwrap().inner_html();
        let probset = element.select(&probset_selector).next().unwrap().inner_html();
        results.push(ProblemSearchResult {
            title,
            url,
            group,
            probset,
            id,
            accepted_cnt,
            submission_cnt: submit_cnt,
        });
    }

    Ok(results)
}
