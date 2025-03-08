mod app;
mod libopenjudge;

use app::{process_credentials, search, submit_solution, test_solution, view_problem};

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

    /// View problems, groups, status.
    View {
        #[command(subcommand)]
        view_type: ViewType,
    },

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

    Search {
        /// Group name, used to construct query url like http://{group}.openjudge.cn/search/?q=...
        #[arg()]
        group: String, 
        /// Search query.
        #[arg()]
        query: String,
    }
}

#[derive(Subcommand)]
enum ViewType {
    User,
    Problem {
        /// URL of the problem.
        /// Use "." to view the last operated problem.
        #[arg()]
        url: String,
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
                todo!("View user status.");
            }
            ViewType::Problem { url } => {
                view_problem(url).await?;
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
    }

    Ok(())
}
