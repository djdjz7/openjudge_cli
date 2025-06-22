use std::{env, fmt::Write, str::FromStr, sync::LazyLock};

use anyhow::Result;
use base64::{Engine, engine::Config, prelude::BASE64_STANDARD};
use colored::Colorize;
use ego_tree::NodeRef;
use image::{DynamicImage, ImageEncoder, ImageReader, codecs::png::PngEncoder};
use markup5ever::local_name;
use onig::Regex;
use scraper::{ElementRef, Node};
use serde::{Deserialize, Serialize};

static WHITESPACE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

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
    let html = scraper::Html::parse_fragment(text);
    let mut output = vec![];
    for child in html.root_element().children() {
        output.push(
            html_to_terminal_output_neo(
                child,
                graphics_protocol,
                false, // do not preserve whitespace by default
            )
            .await,
        );
    }
    output.concat()
}

pub fn shrink_whitespace(text: &str) -> String {
    WHITESPACE_RE.replace_all(text, " ")
}

pub async fn html_to_terminal_output_neo(
    node: NodeRef<'_, Node>,
    graphics_protocol: GraphicsProtocol,
    preserve_whitespace: bool,
) -> String {
    match node.value() {
        Node::Text(text) => {
            if preserve_whitespace {
                text.to_string()
            } else {
                shrink_whitespace(text)
            }
        }
        Node::Document | Node::Fragment => {
            unreachable!("Caller error: call with root_element instead of Document or Fragment.")
        }
        Node::Doctype(_) | Node::Comment(_) | Node::ProcessingInstruction(_) => String::new(),
        Node::Element(_) => {
            let element_ref = ElementRef::wrap(node).unwrap();
            if let local_name!("img") = element_ref.value().name.local {
                return get_image(&element_ref, graphics_protocol).await;
            } else if let local_name!("br") = element_ref.value().name.local {
                return "\n".to_string();
            } else {
                let preserve_whitespace = preserve_whitespace
                    || matches!(element_ref.value().name.local, local_name!("pre"));
                let mut output = vec![];
                for child in element_ref.children() {
                    output.push(
                        Box::pin(html_to_terminal_output_neo(
                            child,
                            graphics_protocol,
                            preserve_whitespace,
                        ))
                        .await,
                    );
                }
                match element_ref.value().name.local {
                    local_name!("b") | local_name!("strong") => output.concat().bold().to_string(),
                    local_name!("h1") => {
                        output.concat().bold().underline().white().to_string() + "\n"
                    }
                    local_name!("h2") => output.concat().bold().underline().to_string() + "\n",
                    local_name!("h3")
                    | local_name!("h4")
                    | local_name!("h5")
                    | local_name!("h6") => output.concat().bold().to_string() + "\n",
                    local_name!("div") => output.concat() + "\n",
                    local_name!("p") => format!("\n{}\n", output.concat()),
                    local_name!("i") | local_name!("em") => output.concat().italic().to_string(),
                    local_name!("mark") => output.concat().black().on_yellow().to_string(),
                    _ => output.concat(),
                }
            }
        }
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
fn encode_image_as_sixel(_img: DynamicImage) -> Result<String> {
    Ok("[No sixel support, please build with sixel feature enabled.]\n".to_string())
}

fn encode_image_as_kitty(img: DynamicImage) -> Result<String> {
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

fn encode_image_as_iterm(img: DynamicImage) -> Result<String> {
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
