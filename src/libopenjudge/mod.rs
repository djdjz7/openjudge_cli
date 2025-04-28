mod selectors;
use anyhow::{Result, anyhow};
use base64::prelude::*;
use reqwest::Client;
use scraper::{self, ElementRef};
use selectors::*;
use serde::{Deserialize, Serialize};

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
    CompileError { message: Option<String> },
    WrongAnswer,
    RuntimeError,
    TimeLimitExceeded,
    OutputLimitExceeded,
    MemoryLimitExceeded,
    Waiting,
    SystemError,
    Unknown,
}

#[derive(PartialEq)]
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
    pub code: String,
    pub submission_time: String,
    pub memory: Option<String>,
    pub time: Option<String>,
}

pub struct ProblemListEntry {
    pub problem_number: String,
    pub title: String,
    pub accepted_population: u32,
    pub submitters: u32,
    pub url: String,
    pub solved: Option<bool>,
}

pub struct ProblemSearchResult {
    pub title: String,
    pub url: String,
    pub group: String,
    pub probset: String,
    pub problem_number: String,
    pub accepted_cnt: u32,
    pub submission_cnt: u32,
}

pub struct User {
    pub id: String,
    pub username: String,
    pub school: String,
    pub sex: String,
    pub register_time: String,
}

pub struct SubmissionHistoryEntry {
    pub result: SubmissionResult,
    pub time: String,
    pub url: String,
}

pub struct Group {
    pub name: String,
    pub description: String,
    pub url: String,
    pub probsets: Vec<ProblemSetEntry>,
}

pub struct ProblemSetEntry {
    pub name: String,
    pub url: String,
}

pub struct ProblemSetPartial {
    pub name: String,
    pub group_name: String,
    pub url: String,
    pub page: u32,
    pub max_page: u32,
    pub problems: Vec<ProblemListEntry>,
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

async fn get_and_parse_html(http_client: &Client, url: &str) -> Result<scraper::Html> {
    let html = http_client.get(url).send().await?.text().await?;
    Ok(scraper::html::Html::parse_document(&html))
}

pub async fn get_problem(http_client: &Client, url: &str) -> Result<Problem> {
    let dom = get_and_parse_html(http_client, url).await?;
    let group = query_selector_inner_text(&dom, &PAGE_HEADER_GROUP_SELECTOR);
    let probset = query_selector_inner_text(&dom, &PAGE_HEADER_PROBSET_SELECTOR);
    let problem_content_dts = dom
        .select(&PROBLEM_PAGE_CONTENT_DTS_SELECTOR)
        .collect::<Vec<_>>();
    let title = query_selector_inner_text(&dom, &PROBLEM_PAGE_TITLE_SELECTOR);
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
            let dd_text = ElementRef::wrap(dd).unwrap().html();
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
        return Err(anyhow!("Login Failed: {}", response.status().to_string()));
    }
    let response_text = response.text().await?;
    let response: LoginResponse = serde_json::from_str(&response_text)?;
    if response.result != "SUCCESS" {
        return Err(anyhow!(
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
    let dom = get_and_parse_html(http_client, &url).await?;
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
        return Err(anyhow!(
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
        return Err(anyhow!(
            response
                .message
                .unwrap_or_else(|| "No message provided".to_string())
        ));
    }
    if response.redirect.is_none() {
        return Err(anyhow!("No redirect URL provided."));
    }
    let redirect_url = response.redirect.unwrap();
    Ok(redirect_url)
}

pub async fn query_submission_result(
    http_client: &Client,
    result_page_url: &str,
) -> Result<Submission> {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    // this finishes instantly
    interval.tick().await;
    loop {
        let dom = get_and_parse_html(http_client, result_page_url).await?;
        let status = query_selector_inner_text(&dom, &COMPILE_STATUS_SELECTOR);
        if status == "Waiting" {
            interval.tick().await;
        } else {
            let result = match status.as_str() {
                "Accepted" => SubmissionResult::Accepted,
                "Compile Error" => {
                    let message = query_selector_inner_text(&dom, &COMPILER_INFO_SELECTOR);
                    SubmissionResult::CompileError {
                        message: Some(message),
                    }
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
                .select(&SUBMISSION_DETAILS_DTS_SELECTOR)
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

            let code = query_selector_inner_text(&dom, &SUBMISSION_CODE_SELECTOR);

            return Ok(Submission {
                result,
                id,
                author,
                lang,
                code,
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
    let url = format!("http://{}.openjudge.cn/search/?q={}", group, query);
    let dom = get_and_parse_html(http_client, &url).await?;
    let mut results = Vec::new();
    for element in dom.select(&PROBLEM_LIST_ROW) {
        let title_anchor = element.select(&ROW_TITLE_SELECTOR).next().unwrap();
        let title = title_anchor.inner_html();
        let url = title_anchor.value().attr("href").unwrap().to_string();
        let id = element
            .select(&ROW_NUMBER_SELECTOR)
            .next()
            .unwrap()
            .inner_html();
        let accepted_cnt = element
            .select(&ROW_ACCEPTED_CNT_SELECTOR)
            .next()
            .unwrap()
            .inner_html()
            .parse()?;
        let submit_cnt = element
            .select(&ROW_SUBMISSION_CNT_SELECTOR)
            .next()
            .unwrap()
            .inner_html()
            .parse()?;
        let group = element
            .select(&ROW_GROUP_SELECTOR)
            .next()
            .unwrap()
            .inner_html();
        let probset = element
            .select(&ROW_PROBSET_SELECTOR)
            .next()
            .unwrap()
            .inner_html();
        results.push(ProblemSearchResult {
            title,
            url,
            group,
            probset,
            problem_number: id,
            accepted_cnt,
            submission_cnt: submit_cnt,
        });
    }

    Ok(results)
}

pub async fn get_user_info(http_client: &Client) -> Result<User> {
    let dom = get_and_parse_html(http_client, "http://openjudge.cn/").await?;
    let user_homepage_anchor = dom
        .select(&USER_HOMEPAGE_SELECTOR)
        .next()
        .ok_or(anyhow!("Cannot select element for user homepage"))?;
    if user_homepage_anchor.inner_html() != "个人首页" {
        return Err(anyhow!(
            "Selected user homepage anchor element does not seem to be correct. Selected value: {}, expected value: {}",
            user_homepage_anchor.inner_html(),
            "个人首页"
        ));
    }
    let homepage_url = user_homepage_anchor.attr("href").ok_or(anyhow!(
        "Selected user homepage anchor does not contain href attribute."
    ))?;
    let id = homepage_url
        .trim_end_matches('/')
        .split('/')
        .last()
        .ok_or(anyhow!("Cannot strip user id from user homepage url."))?
        .to_string();

    let dom = get_and_parse_html(http_client, homepage_url).await?;

    let username = query_selector_inner_text(&dom, &USERHOME_NAME_SELECTOR);
    let sex = query_selector_inner_text(&dom, &USERHOME_SEX_SELECTOR);
    let school = query_selector_inner_text(&dom, &USERHOME_SCHOOL_SELECTOR);
    let register_time = query_selector_inner_text(&dom, &USERHOME_REGISTER_TIME_SELECTOR);

    Ok(User {
        id,
        username,
        sex,
        school,
        register_time,
    })
}

pub async fn list_submissions(
    http_client: &Client,
    prob_url: &str,
) -> Result<Vec<SubmissionHistoryEntry>> {
    let dom = get_and_parse_html(http_client, prob_url).await?;
    let entries = dom
        .select(&PROBLEM_PAGE_SOLUTION_ROW_SELECTOR)
        .collect::<Vec<_>>();
    let mut results = Vec::<SubmissionHistoryEntry>::new();
    for entry in entries {
        let result_anchor = entry.select(&ROW_RESULT_SELECTOR).next().ok_or(anyhow!(
            "Cannot select result anchor element in submission list entry."
        ))?;
        let result = result_anchor.inner_html();
        let url = result_anchor
            .value()
            .attr("href")
            .ok_or(anyhow!(
                "Selected result anchor does not contain href attribute."
            ))?
            .to_string();
        let time = entry
            .select(&ROW_TIME_SELECTOR)
            .next()
            .unwrap()
            .inner_html();
        results.push(SubmissionHistoryEntry {
            result: match result.as_str() {
                "Accepted" => SubmissionResult::Accepted,
                "Compile Error" => SubmissionResult::CompileError { message: None },
                "Presentation Error" => SubmissionResult::PresentationError,
                "Wrong Answer" => SubmissionResult::WrongAnswer,
                "Runtime Error" => SubmissionResult::RuntimeError,
                "Time Limit Exceeded" => SubmissionResult::TimeLimitExceeded,
                "Output Limit Exceeded" => SubmissionResult::OutputLimitExceeded,
                "Memory Limit Exceeded" => SubmissionResult::MemoryLimitExceeded,
                "Waiting" => SubmissionResult::Waiting,
                "System Error" => SubmissionResult::SystemError,
                _ => SubmissionResult::Unknown,
            },
            time,
            url,
        })
    }
    Ok(results)
}

pub async fn get_group_info(http_client: &Client, group: &str) -> Result<Group> {
    let url = format!("http://{}.openjudge.cn/", group);
    let dom = get_and_parse_html(http_client, &url).await?;
    let anchors = dom
        .select(&GROUP_PAGE_PROBSET_ANCHORS_SELECTOR)
        .collect::<Vec<_>>();
    let group_name = query_selector_inner_text(&dom, &GROUP_PAGE_NAME_SELECTOR);
    let group_description = query_selector_inner_text(&dom, &GROUP_PAGE_DESCRIPTION_SELECTOR);
    let mut probsets = Vec::new();
    for anchor in &anchors {
        let name = anchor.inner_html();
        let url = anchor
            .value()
            .attr("href")
            .ok_or(anyhow!(
                "Selected probset anchor does not contain href attribute."
            ))?
            .to_string();
        probsets.push(ProblemSetEntry { name, url });
    }
    Ok(Group {
        name: group_name,
        description: group_description,
        url,
        probsets,
    })
}

pub async fn get_partial_probset_info(
    http_client: &Client,
    group: &str,
    probset: &str,
    page: Option<u32>,
) -> Result<ProblemSetPartial> {
    let url = match page {
        Some(page) => format!("http://{}.openjudge.cn/{}/?page={}", group, probset, page),
        None => format!("http://{}.openjudge.cn/{}/", group, probset),
    };
    let dom = get_and_parse_html(http_client, &url).await?;
    let entries = dom.select(&PROBSET_PROBLEM_ROW).collect::<Vec<_>>();
    let mut problems = Vec::new();
    for entry in entries {
        let problem_number = entry
            .select(&PROBSET_ROW_NUMBER_SELECTOR)
            .next()
            .ok_or(anyhow!("Cannot select problem number element."))?
            .text()
            .collect::<Vec<_>>()
            .concat();
        let title_anchor = entry
            .select(&PROBSET_ROW_TITLE_SELECTOR)
            .next()
            .ok_or(anyhow!("Cannot select title element."))?;
        let title = title_anchor.inner_html();
        let url = title_anchor
            .value()
            .attr("href")
            .ok_or(anyhow!("Cannot find href attribute on title anchor."))?
            .to_string();
        let accepted_population = entry
            .select(&PROBSET_ROW_ACCEPTED_CNT_SELECTOR)
            .next()
            .ok_or(anyhow!("Cannot select AC population element."))?
            .inner_html()
            .parse()?;
        let submit_population = entry
            .select(&PROBSET_ROW_SUBMISSION_CNT_SELECTOR)
            .next()
            .ok_or(anyhow!("Cannot select submission population element."))?
            .inner_html()
            .parse()?;
        let solved = entry
            .select(&PROBSET_ROW_SOLVED_TD_SELECTOR)
            .next()
            .map(|td| td.select(&PROBSET_ROW_SOLVED_IMG_SELECTOR).next().is_some());
        problems.push(ProblemListEntry {
            problem_number,
            title,
            accepted_population,
            submitters: submit_population,
            url,
            solved,
        });
    }
    let probset_name = query_selector_inner_text(&dom, &PAGE_HEADER_PROBSET_SELECTOR);
    let group_name = query_selector_inner_text(&dom, &PAGE_HEADER_GROUP_SELECTOR);
    let page = dom
        .select(&PAGEBAR_CURRENT_SELECTOR)
        .next()
        .map_or(1, |element| element.inner_html().parse().unwrap_or(1u32));
    let max_page = dom
        .select(&PAGEBAR_LAST_SELECTOR)
        .next()
        .map_or(1, |element| element.inner_html().parse().unwrap_or(1u32));
    Ok(ProblemSetPartial {
        problems,
        name: probset_name,
        group_name,
        url,
        page,
        max_page,
    })
}
