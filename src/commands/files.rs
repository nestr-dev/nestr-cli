use std::path::{Path, PathBuf};

use anyhow::{bail, Context as _, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use clap::Subcommand;
use serde_json::{json, Value};

use crate::api_client::{unwrap_data, NestrClient};
use crate::commands::GlobalArgs;
use crate::config::{OutputFormat, ResolvedConfig};
use crate::render::{self, print_json};
use crate::safety;

#[derive(Subcommand)]
pub enum FilesCmd {
    /// List the files attached to a nest (a comment id works too).
    List {
        nest_id: String,
        /// Only files uploaded under this group id (the uploading field's id).
        #[arg(long, value_name = "GROUP")]
        context: Option<String>,
        /// Only files whose extracted text matches, with the matching passages.
        #[arg(long)]
        search: Option<String>,
    },
    /// Read one file: prints a text file, saves any file with --save.
    Read {
        nest_id: String,
        file_id: String,
        /// Write the file's bytes to this path instead of printing them.
        #[arg(long, value_name = "PATH", conflicts_with = "text")]
        save: Option<PathBuf>,
        /// Overwrite the --save path if it already exists.
        #[arg(long, requires = "save")]
        force: bool,
        /// Read the extracted plain text (documents whose text was extracted on upload).
        #[arg(long)]
        text: bool,
        /// With --text, the character offset to read from (a previous nextOffset).
        #[arg(long, requires = "text")]
        offset: Option<u64>,
    },
    /// Upload a local file and attach it to a nest (a comment id works too).
    Upload {
        nest_id: String,
        /// The local file to upload.
        path: PathBuf,
        /// File name to store (defaults to the path's file name).
        #[arg(long)]
        name: Option<String>,
        /// MIME type (guessed from the extension when omitted).
        #[arg(long)]
        content_type: Option<String>,
    },
    /// Delete a file from a nest (permanent).
    Delete { nest_id: String, file_id: String },
}

pub async fn list_files(
    client: &NestrClient,
    nest_id: &str,
    context: Option<&str>,
    search: Option<&str>,
) -> crate::error::Result<Value> {
    let mut params: Vec<(&str, &str)> = Vec::new();
    if let Some(c) = context {
        params.push(("context", c));
    }
    if let Some(s) = search {
        params.push(("search", s));
    }
    let raw: Value = client
        .get(&format!("/nests/{nest_id}/files"), &params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// The file's descriptor plus its bytes in `dataBase64`.
pub async fn read_file(
    client: &NestrClient,
    nest_id: &str,
    file_id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .get(&format!("/nests/{nest_id}/files/{file_id}"), &[])
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// The extracted text (`as=text`): `{text, offset, total, nextOffset}`, a slice when large.
pub async fn read_file_text(
    client: &NestrClient,
    nest_id: &str,
    file_id: &str,
    offset: Option<u64>,
) -> crate::error::Result<Value> {
    let offset = offset.map(|o| o.to_string());
    let mut params: Vec<(&str, &str)> = vec![("as", "text")];
    if let Some(o) = &offset {
        params.push(("offset", o));
    }
    let raw: Value = client
        .get(&format!("/nests/{nest_id}/files/{file_id}"), &params)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn upload_file(
    client: &NestrClient,
    nest_id: &str,
    name: &str,
    content_type: &str,
    bytes: &[u8],
) -> crate::error::Result<Value> {
    let body = json!({
        "name": name,
        "contentType": content_type,
        "dataBase64": STANDARD.encode(bytes),
    });
    let raw: Value = client
        .post(&format!("/nests/{nest_id}/files"), &body)
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

pub async fn delete_file(
    client: &NestrClient,
    nest_id: &str,
    file_id: &str,
) -> crate::error::Result<Value> {
    let raw: Value = client
        .delete(&format!("/nests/{nest_id}/files/{file_id}"))
        .await?;
    let (data, _, _) = unwrap_data(raw);
    Ok(data)
}

/// Decode a read response's `dataBase64`.
pub fn decode_bytes(file: &Value) -> Result<Vec<u8>> {
    let b64 = file
        .get("dataBase64")
        .and_then(Value::as_str)
        .context("the response carried no file content")?;
    STANDARD
        .decode(b64)
        .context("the file content is not valid base64")
}

/// MIME type for a path, by extension. Unknown or missing → `application/octet-stream`.
pub fn guess_content_type(path: impl AsRef<Path>) -> &'static str {
    let ext = path
        .as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "txt" | "text" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

/// Content types safe to print as text (after a UTF-8 check).
fn is_textual(content_type: &str) -> bool {
    let ct = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    ct.starts_with("text/")
        || ct.ends_with("+json")
        || ct.ends_with("+xml")
        || matches!(
            ct.as_str(),
            "application/json" | "application/xml" | "application/yaml" | "application/toml"
        )
}

pub fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    let f = n as f64;
    if f < KB {
        format!("{n} B")
    } else if f < KB * KB {
        format!("{:.1} KB", f / KB)
    } else {
        format!("{:.1} MB", f / (KB * KB))
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

/// File descriptors as a table; with search matches, each file's passages below it.
pub fn output_files(data: &Value, output: OutputFormat, empty: &str) -> Result<()> {
    if output == OutputFormat::Json {
        return print_json(data);
    }
    let files = data.as_array().cloned().unwrap_or_default();
    if files.is_empty() {
        render::print_no_results(empty);
        return Ok(());
    }
    let has_matches = files.iter().any(|f| f.get("matches").is_some());
    if has_matches {
        for f in &files {
            println!(
                "{}  [{}]",
                render::clean_text(str_of(f, "name")),
                render::clean_text(str_of(f, "id"))
            );
            for m in f
                .get("matches")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let offset = m.get("offset").and_then(Value::as_u64).unwrap_or(0);
                println!("  @{offset}: {}", render::clean_text(str_of(m, "excerpt")));
            }
        }
        return Ok(());
    }
    let rows: Vec<Vec<String>> = files
        .iter()
        .map(|f| {
            vec![
                render::clean_text(str_of(f, "id")),
                render::clean_text(str_of(f, "name")),
                render::clean_text(str_of(f, "contentType")),
                format_bytes(f.get("size").and_then(Value::as_u64).unwrap_or(0)),
            ]
        })
        .collect();
    println!(
        "{}",
        render::format_table(&["ID", "NAME", "TYPE", "SIZE"], rows)
    );
    Ok(())
}

/// Print an `as=text` slice, pointing at the next offset when there is more.
pub fn output_text_slice(data: &Value, output: OutputFormat, more_cmd: &str) -> Result<()> {
    if output == OutputFormat::Json {
        return print_json(data);
    }
    let text = str_of(data, "text");
    print!("{}", render::sanitize(text));
    if !text.ends_with('\n') {
        println!();
    }
    if let Some(next) = data.get("nextOffset").and_then(Value::as_u64) {
        let total = data.get("total").and_then(Value::as_u64).unwrap_or(0);
        eprintln!(
            "(more: {next} of {total} characters read; continue with `{more_cmd} --offset {next}`)"
        );
    }
    Ok(())
}

fn without_bytes(file: &Value) -> Value {
    let mut v = file.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("dataBase64");
    }
    v
}

fn write_new_file(dest: &Path, bytes: &[u8], force: bool) -> Result<()> {
    if !force && dest.exists() {
        bail!(
            "{} already exists; pass --force to overwrite it.",
            dest.display()
        );
    }
    std::fs::write(dest, bytes).with_context(|| format!("writing {}", dest.display()))
}

pub async fn run(
    cmd: FilesCmd,
    cfg: &ResolvedConfig,
    client: &NestrClient,
    g: &GlobalArgs,
) -> Result<()> {
    match cmd {
        FilesCmd::List {
            nest_id,
            context,
            search,
        } => {
            let data = list_files(client, &nest_id, context.as_deref(), search.as_deref()).await?;
            let empty = if search.is_some() {
                "No file matches.".to_string()
            } else {
                format!("No files on {nest_id}.")
            };
            output_files(&data, cfg.output, &empty)?;
        }
        FilesCmd::Read {
            nest_id,
            file_id,
            save,
            force,
            text,
            offset,
        } => {
            if text {
                let data = read_file_text(client, &nest_id, &file_id, offset).await?;
                let more = format!("nestr nests files read {nest_id} {file_id} --text");
                return output_text_slice(&data, cfg.output, &more);
            }
            let file = read_file(client, &nest_id, &file_id).await?;
            let name = render::clean_text(str_of(&file, "name"));
            let content_type = str_of(&file, "contentType").to_string();
            let size = file.get("size").and_then(Value::as_u64).unwrap_or(0);
            if let Some(dest) = save {
                let bytes = decode_bytes(&file)?;
                write_new_file(&dest, &bytes, force)?;
                match cfg.output {
                    OutputFormat::Json => {
                        let mut v = without_bytes(&file);
                        v["savedTo"] = json!(dest.display().to_string());
                        print_json(&v)?;
                    }
                    OutputFormat::Text => println!(
                        "Saved {name} ({}) to {}",
                        format_bytes(bytes.len() as u64),
                        dest.display()
                    ),
                }
                return Ok(());
            }
            if cfg.output == OutputFormat::Json {
                return print_json(&file);
            }
            let decoded = if is_textual(&content_type) {
                decode_bytes(&file)
                    .ok()
                    .and_then(|b| String::from_utf8(b).ok())
            } else {
                None
            };
            match decoded {
                Some(s) => {
                    print!("{}", render::sanitize(&s));
                    if !s.ends_with('\n') {
                        println!();
                    }
                }
                None => {
                    println!(
                        "{name}  ({}, {})",
                        render::clean_text(&content_type),
                        format_bytes(size)
                    );
                    println!(
                        "Not printable as text; save it with `nestr nests files read {nest_id} {file_id} --save <path>`."
                    );
                }
            }
        }
        FilesCmd::Upload {
            nest_id,
            path,
            name,
            content_type,
        } => {
            safety::enforce_read_only(g.read_only, "nests files upload")?;
            let name = match name {
                Some(n) => n,
                None => path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_string)
                    .with_context(|| format!("{} has no file name; pass --name", path.display()))?,
            };
            let content_type =
                content_type.unwrap_or_else(|| guess_content_type(&name).to_string());
            let bytes =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let data = upload_file(client, &nest_id, &name, &content_type, &bytes).await?;
            match cfg.output {
                OutputFormat::Json => print_json(&data)?,
                OutputFormat::Text => println!(
                    "Uploaded {} ({}) to {nest_id}  [{}]",
                    render::clean_text(str_of(&data, "name")),
                    format_bytes(data.get("size").and_then(Value::as_u64).unwrap_or(0)),
                    render::clean_text(str_of(&data, "id"))
                ),
            }
        }
        FilesCmd::Delete { nest_id, file_id } => {
            safety::enforce_read_only(g.read_only, "nests files delete")?;
            safety::confirm_destructive(
                &format!("Delete file '{file_id}' from '{nest_id}'?"),
                g.yes,
            )?;
            delete_file(client, &nest_id, &file_id).await?;
            match cfg.output {
                OutputFormat::Json => print_json(&json!({"deleted": file_id, "nestId": nest_id}))?,
                OutputFormat::Text => println!("Deleted file {file_id} from {nest_id}."),
            }
        }
    }
    Ok(())
}
