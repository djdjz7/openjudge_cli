mod app;
mod display;
mod libopenjudge;

use app::*;

use anyhow::Result;
use clap::{Parser, Subcommand, arg, command};

const NAME: &str = "OpenJudge CLI";
const VERSION: &str = "0.0.1";
const ABOUT: &str = "CLI for OpenJudge (openjudge.cn)";

#[derive(Parser)]
#[command(name = NAME, version = VERSION, author, about = ABOUT, long_about = ABOUT)]
struct Cli {
    #[command(subcommand)]
    command: AppCommand,
}

#[derive(Subcommand)]
enum AppCommand {
    #[command(visible_alias = "auth")]
    /// Save credentials to local keyring.
    Credentials {
        #[arg()]
        email: String,
    },

    #[command(visible_alias = "v")]
    /// View problems, groups, status.
    View {
        #[command(subcommand)]
        view_type: ViewType,
    },

    #[command(visible_alias = "s")]
    /// Submit a solution to a problem.
    Submit {
        /// URL of the problem, excluding '/submit'.
        /// Use "." to submit to the last operated problem.
        #[arg()]
        url: String,
        /// Path to the source code file.
        #[arg()]
        file: String,
        /// Language of the source code file, overrides inferred language.
        /// Supported values (case insensitive):
        /// - C, GCC;
        /// - C++, G++;
        /// - Py, Python, Py3, Python3;
        /// - PyPy, PyPy3.
        #[arg(short, long)]
        lang: Option<String>,
    },

    #[command(visible_alias = "t")]
    /// Test a solution against sample input/output.
    /// Be aware: testing solution locally requires compiler/interpreter be accessible via command line.
    /// For C, gcc is called;
    /// For C++, g++ is called;
    /// For Python, python3 is called;
    /// For PyPy, pypy3 is called.
    Test {
        /// URL of the problem.
        /// Use "." to test the last operated problem.
        #[arg()]
        url: String,
        /// Path to the source code file.
        #[arg()]
        file: String,
        /// Language of the source code file, overrides inferred language.
        /// Supported values (case insensitive):
        /// - C, GCC;
        /// - C++, G++;
        /// - Py, Python, Py3, Python3;
        /// - PyPy, PyPy3.
        #[arg(short, long)]
        lang: Option<String>,
        /// Proceed to submit if accepted.
        #[arg(short, long)]
        submit: bool,
    },

    #[command(visible_alias = "S")]
    /// Use keyword to search within a group.
    Search {
        /// Group name, used to construct query url like http://{group}.openjudge.cn/search/?q=...
        #[arg()]
        group: String,
        /// Search query.
        #[arg()]
        query: String,
    },

    #[command(visible_alias = "l")]
    /// List submissions, problem sets, problems.
    List {
        #[command(subcommand)]
        list_type: ListType,
    },
}

#[derive(Subcommand)]
enum ViewType {
    User,

    #[command(alias = "p")]
    Problem {
        /// URL of the problem.
        /// Use "." to view the last operated problem.
        #[arg()]
        url: String,
    },
    Submission {
        #[arg()]
        url: String,
    },
}

#[derive(Subcommand)]
enum ListType {
    /// List all submissions commited by user of a problem.
    #[command(visible_alias = "s")]
    Submissions {
        #[arg()]
        problem_url: String,
    },

    /// List all problem sets under a certain group.
    /// Requests are constructed as http://{group}.openjudge.cn/
    #[command(visible_alias = "P")]
    Probsets {
        #[arg()]
        group: String,
    },

    /// List all problems under a problem set.
    /// Requests are constructed as http://{group}.openjudge.cn/{probset}/
    #[command(visible_alias = "p")]
    Problems {
        #[arg()]
        group: String,
        #[arg()]
        probset: String,
        #[arg()]
        page: Option<u32>,
        #[arg(short = 's', long = "status")]
        show_status: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        AppCommand::Credentials { email } => {
            process_credentials(email).await?;
        }
        AppCommand::View { view_type } => match view_type {
            ViewType::User => {
                view_user().await?;
            }
            ViewType::Problem { url } => {
                view_problem(&url).await?;
            }
            ViewType::Submission { url } => {
                view_submission(&url).await?;
            }
        },
        AppCommand::Submit { url, file, lang } => {
            submit_solution(&url, &file, lang).await?;
        }
        AppCommand::Test {
            url,
            file,
            lang,
            submit,
        } => {
            test_solution(&url, &file, lang, submit).await?;
        }
        AppCommand::Search { group, query } => {
            search(&group, &query).await?;
        }
        AppCommand::List { list_type } => match list_type {
            ListType::Submissions { problem_url } => {
                list_submissions(&problem_url).await?;
            }
            ListType::Probsets { group } => {
                list_probsets(&group).await?;
            }
            ListType::Problems {
                group,
                probset,
                page,
                show_status,
            } => {
                list_problems(&group, &probset, page, show_status).await?;
            }
        },
    }

    Ok(())
}
