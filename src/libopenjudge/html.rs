use colored::Colorize;
use image::ImageReader;
use markup5ever::local_name;
use scraper::ElementRef;
use sixel_bytes;

pub async fn html_to_terminal_output(element: &ElementRef<'_>, enable_sixel: bool) -> String {
    if let local_name!("pre") = element.value().name.local {
        return element.text().collect();
    }
    let mut output: Vec<String> = vec![];
    for child in element.children() {
        if child.value().is_comment() || child.value().is_doctype() {
            continue;
        }
        if child.value().is_text() {
            output.push(
                child
                    .value()
                    .as_text()
                    .expect("Reported text node cannot be converted into Text.")
                    .trim()
                    .to_string()
            );
        } else if child.value().is_element() {
            let ele_ref = ElementRef::wrap(child)
                .expect("Reported element node cannot be wrapped as ElementRef.");
            // Use Box::pin to handle recursion in async function
            output.push(Box::pin(html_to_terminal_output(&ele_ref, enable_sixel)).await);
        }
    }
    match element.value().name.local.to_ascii_lowercase() {
        local_name!("b") | local_name!("strong") => output.concat().bold().white().to_string(),
        local_name!("h1") => output.concat().bold().underline().white().to_string() + "\n",
        local_name!("h2") => output.concat().bold().underline().to_string() + "\n",
        local_name!("h3") | local_name!("h4") | local_name!("h5") | local_name!("h6") => {
            output.concat().bold().to_string() + "\n"
        }
        local_name!("div") => output.concat() + "\n",
        local_name!("p") => format!("\n{}\n", output.concat()),
        local_name!("i") | local_name!("em") => output.concat().italic().to_string(),
        local_name!("mark") => output.concat().black().on_yellow().to_string(),
        local_name!("br") => "\n".to_string(),
        local_name!("img") => get_sixel(&element, enable_sixel).await,
        _ => output.concat(),
    }
}

async fn get_sixel(img: &ElementRef<'_>, enable_sixel: bool) -> String {
    let src = img.attr("src");
    if src.is_none() {
        return "".to_string();
    }
    let src = src.unwrap().trim();
    if !enable_sixel {
        return format!("[Image src {}]\n", src);
    }
    let client = reqwest::Client::new();
    let resp = client.get(src).send().await;
    if resp.is_err() {
        return format!("[Image src {} fetch failed]", src);
    }
    let bytes = resp.unwrap().bytes().await;
    if bytes.is_err() {
        return format!("[Image src {} read bytes failed]", src);
    }
    let bytes = bytes.unwrap();
    ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map(|reader| {
            reader
                .decode()
                .map(|image| {
                    let image = image.into_rgb8();
                    let bytes = image.as_raw();
                    sixel_bytes::sixel_string(
                        bytes,
                        image.width() as _,
                        image.height() as _,
                        sixel_bytes::PixelFormat::RGB888,
                        sixel_bytes::DiffusionMethod::Auto,
                    )
                    .unwrap_or_else(|_| format!("[Image src {} cannot encode as sixel]", src))
                })
                .unwrap_or_else(|_| format!("[Image src {} cannot be decoded]", src))
        })
        .unwrap_or_else(|_| format!("[Image src {} cannot guess format]", src))
}
