use std::{env, error::Error, fmt::Write, str::FromStr};

use base64::{Engine, engine::Config, prelude::BASE64_STANDARD};
use colored::Colorize;
use image::{DynamicImage, ImageEncoder, ImageReader, codecs::png::PngEncoder};
use markup5ever::local_name;
use scraper::ElementRef;
use serde::{Deserialize, Serialize};

#[cfg(feature = "sixel")]
use sixel_bytes;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum GraphicsProtocol {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(rename = "sixel")]
    Sixel,
    #[serde(rename = "kitty")]
    Kitty,
    #[serde(rename = "iterm")]
    ITerm,
    #[serde(rename = "auto")]
    Auto,
}

impl FromStr for GraphicsProtocol {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "n" | "0" | "none" | "disabled" => Ok(GraphicsProtocol::Disabled),
            "s" | "sixel" => use_sixel(),
            "k" | "kitty" => Ok(GraphicsProtocol::Kitty),
            "i" | "iterm" => Ok(GraphicsProtocol::ITerm),
            "a" | "auto" => Ok(GraphicsProtocol::Auto),
            _ => Err(anyhow::format_err!(
                "Invalid value for GraphicsProtocol: {}",
                value
            )),
        }
    }
}

#[cfg(feature = "sixel")]
fn use_sixel() -> Result<GraphicsProtocol, anyhow::Error> {
    return Ok(GraphicsProtocol::Sixel);
}

#[cfg(not(feature = "sixel"))]
fn use_sixel() -> Result<GraphicsProtocol, anyhow::Error> {
    Err(anyhow::format_err!(
        "Sixel feature is not enabled. Please build with the `sixel` feature."
    ))
}

pub async fn get_printable_html_text(text: &str, graphics_protocol: GraphicsProtocol) -> String {
    html_to_terminal_output(
        &scraper::Html::parse_fragment(text).root_element(),
        graphics_protocol,
    )
    .await
}

pub async fn html_to_terminal_output(
    element: &ElementRef<'_>,
    graphics_protocol: GraphicsProtocol,
) -> String {
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
                    .to_string(),
            );
        } else if child.value().is_element() {
            let ele_ref = ElementRef::wrap(child)
                .expect("Reported element node cannot be wrapped as ElementRef.");
            // Use Box::pin to handle recursion in async function
            output.push(Box::pin(html_to_terminal_output(&ele_ref, graphics_protocol)).await);
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
        local_name!("img") => get_image(element, graphics_protocol).await,
        _ => output.concat(),
    }
}

async fn get_image(img: &ElementRef<'_>, graphics_protocol: GraphicsProtocol) -> String {
    let src = img.attr("src");
    if src.is_none() {
        return "".to_string();
    }
    let src = src.unwrap().trim();
    let graphics_protocol = transform_protocol(graphics_protocol);
    if let GraphicsProtocol::Disabled = graphics_protocol {
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
                .map(|image| match graphics_protocol {
                    GraphicsProtocol::Disabled => unreachable!(),
                    GraphicsProtocol::Sixel => encode_image_as_sixel(image).unwrap_or_else(|_| {
                        format!("[Image src {} cannot be encoded as sixel]", src)
                    }),
                    GraphicsProtocol::Kitty => encode_image_as_kitty(image).unwrap_or_else(|_| {
                        format!(
                            "[Image src {} cannot be encoded into kitty image protocol]",
                            src
                        )
                    }),
                    GraphicsProtocol::ITerm => encode_image_as_iterm(image).unwrap_or_else(|_| {
                        format!(
                            "[Image src {} cannot be encoded into iTerm inline image]",
                            src
                        )
                    }),
                    GraphicsProtocol::Auto => unreachable!(),
                })
                .unwrap_or_else(|_| format!("[Image src {} cannot be decoded]", src))
        })
        .unwrap_or_else(|_| format!("[Image src {} cannot guess format]", src))
}

#[cfg(feature = "sixel")]
fn encode_image_as_sixel(img: DynamicImage) -> Result<String, ()> {
    let rgb_image = img.into_rgb8();
    let bytes = rgb_image.as_raw();
    sixel_bytes::sixel_string(
        bytes,
        rgb_image.width() as _,
        rgb_image.height() as _,
        sixel_bytes::PixelFormat::RGB888,
        sixel_bytes::DiffusionMethod::Auto,
    )
    .map_err(|_| ())
}

#[cfg(not(feature = "sixel"))]
fn encode_image_as_sixel(_img: DynamicImage) -> Result<String, ()> {
    Ok("[No sixel support, please build with sixel feature enabled.]\n".to_string())
}

fn encode_image_as_kitty(img: DynamicImage) -> Result<String, ()> {
    Ok(get_image_kitty_data(img).join(""))
}

fn get_image_kitty_data(img: DynamicImage) -> Vec<String> {
    let rgb_image = img.to_rgb8();
    let rgb_data: Vec<u8> = rgb_image.pixels().flat_map(|pix| pix.0).collect();
    let pixels_encoded = BASE64_STANDARD.encode(rgb_data);
    // payload size shall not exceed 4096 bytes, or 4096 chars in ascii.
    // no need to split if len <= 4096.
    if pixels_encoded.len() <= 4096 {
        return vec![format!(
            "\x1b_Gf=24,s={},v={},a=T;{}\x1b\\",
            rgb_image.width(),
            rgb_image.height(),
            pixels_encoded
        )];
    }
    let mut chunk_cnt = pixels_encoded.len() / 4096;
    if chunk_cnt * 4096 != pixels_encoded.len() {
        chunk_cnt += 1
    }
    let mut result = vec![format!(
        "\x1b_Gf=24,s={},v={},a=T,m=1;{}\x1b\\",
        rgb_image.width(),
        rgb_image.height(),
        // since encoded base64 is guaranteed to be ascii
        // slicing will be fine.
        &pixels_encoded[..4096]
    )];

    pixels_encoded[4096..]
        .as_bytes()
        .chunks(4096)
        .enumerate()
        .for_each(|(i, chunk)| {
            // -1 for 0-based index, another -1 for first chunk
            let is_last_chunk = i == chunk_cnt - 2;
            let m_value = if is_last_chunk { "0" } else { "1" };
            let chunk_str = unsafe { std::str::from_utf8_unchecked(chunk) };
            result.push(format!("\x1b_Gm={};{}\x1b\\", m_value, chunk_str));
        });

    result
}

fn encode_image_as_iterm(img: DynamicImage) -> Result<String, Box<dyn Error>> {
    let mut bytes = vec![];
    let (w, h) = (img.width(), img.height());
    PngEncoder::new(&mut bytes).write_image(
        &img.into_rgba8(),
        w,
        h,
        image::ExtendedColorType::Rgba8,
    )?;
    let mut buf = String::with_capacity(
        200 + base64::encoded_len(bytes.len(), BASE64_STANDARD.config().encode_padding())
            .unwrap_or(0),
    );
    write!(
        buf,
        "\x1b]1337;File=inline=1;size={};width={w}px;height={h}px;doNotMoveCursor=1:",
        bytes.len(),
    )?;
    BASE64_STANDARD.encode_string(bytes, &mut buf);
    write!(buf, "\x07")?;
    Ok(buf)
}

fn transform_protocol(original: GraphicsProtocol) -> GraphicsProtocol {
    if !matches!(original, GraphicsProtocol::Auto) {
        return original;
    }
    let term = env::var("TERM");
    if let Ok(term) = term {
        if term.contains("kitty") {
            return GraphicsProtocol::Kitty;
        }
    }
    let term_program = env::var("TERM_PROGRAM");
    if term_program.is_err() {
        return GraphicsProtocol::Disabled;
    }
    match term_program.unwrap().as_str() {
        "ghostty" => GraphicsProtocol::Kitty,
        "vscode" | "iTerm.app" => GraphicsProtocol::ITerm,
        _ => GraphicsProtocol::Disabled,
    }
}
