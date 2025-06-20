use colored::Colorize;
use crossterm::{
    cursor::{self, MoveTo},
    event::{self, KeyCode},
    execute, queue,
    style::Print,
    terminal::{self, ClearType},
};
use std::{
    cmp::min,
    io::{stdout, Write},
};

pub fn select_within<'a>(
    prompt: &str,
    options: &'a [&'a str],
    per_option_height: u16,
) -> Option<usize> {
    if options.is_empty() {
        return None;
    }
    let mut selected_index = 0;
    let mut options_offset_rows = 0;
    // prompt, ellipsis top, ellipsis bottom.
    let fixed_rows = 4;
    // prompt, ellipsis top.
    let display_offset_rows = 2;
    let options_len = options.len();
    let mut stdout = stdout();
    terminal::enable_raw_mode().unwrap();
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide).unwrap();
    let result = loop {
        let (_, terminal_rows) = terminal::size().unwrap();
        let scroll_height = terminal_rows as usize - fixed_rows;
        queue!(
            stdout,
            terminal::Clear(ClearType::All),
            MoveTo(0, 0),
            Print(prompt.white().bold())
        )
        .unwrap();
        for (i, option) in options
            .iter()
            .enumerate()
            .skip((options_offset_rows as f32 / per_option_height as f32).floor() as usize)
            .take((scroll_height as f32 / per_option_height as f32).ceil() as usize)
        {
            for (j, line) in option.lines().enumerate().take(per_option_height as usize) {
                if i * per_option_height as usize + j < options_offset_rows {
                    continue;
                }
                if i * per_option_height as usize + j >= options_offset_rows + scroll_height {
                    break;
                }
                queue!(
                    stdout,
                    MoveTo(
                        0,
                        display_offset_rows as u16
                            + (i * per_option_height as usize + j - options_offset_rows) as u16
                    ),
                    terminal::Clear(ClearType::CurrentLine),
                    if i == selected_index {
                        Print(if j == 0 {
                            format!("{}", format!("> {}", line.clear()).green().bold())
                        } else {
                            format!("  {}", line.normal().green().bold())
                        })
                    } else {
                        Print(format!("  {}", line))
                    }
                )
                .unwrap();
            }
        }
        if options_offset_rows > 0 {
            queue!(
                stdout,
                MoveTo(0, 1),
                terminal::Clear(ClearType::CurrentLine),
                Print("  ...")
            )
            .unwrap();
        }
        if options_offset_rows + scroll_height < options_len * per_option_height as usize {
            queue!(
                stdout,
                MoveTo(0, terminal_rows - 2),
                terminal::Clear(ClearType::CurrentLine),
                Print("  ...")
            )
            .unwrap();
        }
        queue!(
            stdout,
            MoveTo(0, terminal_rows - 1),
            terminal::Clear(ClearType::CurrentLine),
            Print("Navigate with arrow keys, Enter to select, 'q' or Ctrl+C to quit"),
        )
        .unwrap();
        stdout.flush().unwrap();
        let e = event::read().unwrap();
        if !e.is_key() {
            continue;
        }
        let key = e.as_key_event().unwrap();
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break None,
            KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                break None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                selected_index = selected_index.saturating_sub(1);
                if (selected_index * per_option_height as usize) < options_offset_rows {
                    options_offset_rows = selected_index * per_option_height as usize;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                selected_index = min(selected_index + 1, options_len - 1);
                if (selected_index + 1) * per_option_height as usize - options_offset_rows
                    >= scroll_height
                {
                    options_offset_rows =
                        (selected_index + 1) * per_option_height as usize - scroll_height;
                }
            }
            KeyCode::Enter => {
                break Some(selected_index);
            }
            _ => continue,
        }
    };
    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show).unwrap();
    terminal::disable_raw_mode().unwrap();
    return result;
}
