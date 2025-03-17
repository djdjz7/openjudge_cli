use crate::libopenjudge::{
    Group, Problem, ProblemListEntry, ProblemSearchResult, ProblemSetPartial, Submission,
    SubmissionHistoryEntry, SubmissionResult, User,
};
use colored::Colorize;
use std::fmt::Display;

pub const NO_CREDENTIALS_FOUND: &str =
    "No user credentials found. Please run `openjudge-cli credentials` first.";
pub const NO_LAST_PROBLEM_FOUND: &str =
    "Do not have a record of the last operated problem. Please specify a problem URL.";

impl Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ID:              {}", self.id.bold())?;
        writeln!(f, "Username:        {}", self.username.bold())?;
        writeln!(f, "Sex:             {}", self.sex.bold())?;
        writeln!(f, "School:          {}", self.school.bold())?;
        writeln!(f, "Registered time: {}", self.register_time.bold())?;
        Ok(())
    }
}

impl Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}/{}\n", self.group, self.probset.bold())?;
        writeln!(f, "{}\n", self.title.on_yellow().bold())?;
        writeln!(f, "{}\n", self.description)?;
        if let Some(ref input) = self.input {
            writeln!(f, "{}", "Input".yellow().bold())?;
            writeln!(f, "{}\n", input)?;
        }
        if let Some(ref output) = self.output {
            writeln!(f, "{}", "Output".yellow().bold())?;
            writeln!(f, "{}\n", output)?;
        }
        if let Some(ref sample_input) = self.sample_input {
            writeln!(f, "{}", "Sample Input".yellow().bold())?;
            writeln!(f, "{}\n", sample_input)?;
        }
        if let Some(ref sample_output) = self.sample_output {
            writeln!(f, "{}", "Sample Output".yellow().bold())?;
            writeln!(f, "{}\n", sample_output)?;
        }
        if let Some(ref hint) = self.hint {
            writeln!(f, "{}", "Hint".yellow().bold())?;
            writeln!(f, "{}\n", hint)?;
        }
        if let Some(ref source) = self.source {
            writeln!(f, "{}", "Source".yellow().bold())?;
            writeln!(f, "{}\n", source)?;
        }
        Ok(())
    }
}

impl Display for ProblemSearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "#{} {} {}/{}",
            self.problem_number,
            self.title.yellow().bold(),
            self.group,
            self.probset.bold()
        )?;
        writeln!(f, "{}", self.url.blue().underline().bold())?;
        writeln!(
            f,
            "{}/Submissions: {}/{}",
            "AC".blue(),
            self.accepted_cnt.to_string().blue(),
            self.submission_cnt
        )?;
        Ok(())
    }
}

impl Display for Submission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.result {
            SubmissionResult::Accepted => {
                writeln!(f, "{}", "Accepted!".blue().bold())?;
            }
            SubmissionResult::CompileError { message } => {
                writeln!(f, "{}", "Compile Error.".green().bold())?;
                writeln!(
                    f,
                    "\n{}\n{}\n",
                    "Compiler Diagnostics:".green(),
                    message
                        .as_ref()
                        .map(|v| v.as_str())
                        .unwrap_or("No message provided.")
                )?;
            }
            _ => {
                writeln!(
                    f,
                    "{}",
                    match self.result {
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
                )?;
            }
        }
        writeln!(f, "#{}", self.id.white().bold())?;
        writeln!(f, "Author:      {}", self.author.white().bold())?;
        writeln!(f, "Lang:        {}", self.lang.white().bold())?;
        if let Some(time) = &self.time {
            writeln!(f, "Time:        {}", time.white().bold())?;
        }
        if let Some(memory) = &self.memory {
            writeln!(f, "Memory:      {}", memory.white().bold())?;
        }
        writeln!(f, "Submit Time: {}", self.submission_time.white().bold())?;
        Ok(())
    }
}

impl Display for SubmissionHistoryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let result = match &self.result {
            SubmissionResult::Accepted => "Accepted".blue().bold(),
            SubmissionResult::CompileError { .. } => "Comp. Err.".green().bold(),
            SubmissionResult::WrongAnswer => "Wrong Ans.".red().bold(),
            SubmissionResult::TimeLimitExceeded => "Time Lim. Ex.".red().bold(),
            SubmissionResult::MemoryLimitExceeded => "Mem. Lim. Ex.".red().bold(),
            SubmissionResult::RuntimeError => "Runtime Err.".red().bold(),
            SubmissionResult::OutputLimitExceeded => "Out. Lim. Ex.".red().bold(),
            SubmissionResult::PresentationError => "Present. Err.".red().bold(),
            _ => "Unknown Err.".red().bold(),
        };
        write!(
            f,
            "{:<13} {} {}",
            result,
            self.time,
            self.url.blue().underline()
        )?;
        Ok(())
    }
}

impl Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.name.bold())?;
        writeln!(f, "{}", self.url.blue().underline())?;
        writeln!(f, "{}", self.description)?;
        writeln!(f)?;
        if self.probsets.is_empty() {
            writeln!(f, "No problem sets found.")?;
        } else {
            writeln!(
                f,
                "Contains {} problem sets:",
                self.probsets.len().to_string().bold()
            )?;
            for probset in &self.probsets {
                writeln!(
                    f,
                    "{} {}",
                    probset.name.bold(),
                    probset.url.blue().underline()
                )?;
            }
        }
        Ok(())
    }
}

impl Display for ProblemListEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{} {} {}",
            match self.solved {
                Some(true) => ("#".to_owned() + &self.problem_number)
                    .blue()
                    .bold(),
                Some(false) => ("#".to_owned() + &self.problem_number)
                    .yellow()
                    .bold(),
                None => ("#".to_owned() + &self.problem_number).bold(),
            },
            self.title.yellow().bold(),
            self.url.blue().underline()
        )?;
        write!(
            f,
            "- {}/Submitters: {}/{}",
            "AC".blue(),
            self.accepted_population.to_string().blue(),
            self.submitters
        )?;
        Ok(())
    }
}

impl Display for ProblemSetPartial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}/{}", self.group_name, self.name.bold())?;
        writeln!(f, "{}\n", self.url.blue().underline())?;
        if self.max_page != 1 {
            writeln!(
                f,
                "Displaying page {} of {}\n",
                self.page.to_string().bold(),
                self.max_page.to_string().bold()
            )?;
        }
        for problem in &self.problems {
            writeln!(f, "{}", problem)?;
        }
        if self.max_page != 1 {
            writeln!(
                f,
                "Displaying page {} of {}",
                self.page.to_string().bold(),
                self.max_page.to_string().bold()
            )?;
        }
        Ok(())
    }
}
