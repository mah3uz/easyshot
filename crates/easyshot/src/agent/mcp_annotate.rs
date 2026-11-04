use super::annotate::{Failure, perform, saved_style};
use super::mcp::{failure, refusal};
use super::mcp_image::preview;
use super::mcp_tools::{self, ToolDef};
use es_protocol::agent::ErrorCode;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject};
use serde_json::{Value, json};
use std::path::PathBuf;

pub(super) async fn call(def: &ToolDef, args: &JsonObject) -> CallToolResult {
    let (job, include_image) = match job(def, args) {
        Ok(parts) => parts,
        Err(message) => return failure(&message, None),
    };
    let done = tokio::task::spawn_blocking(move || {
        perform(&job.image, &job.ops, &job.out, job.force, &saved_style())
    })
    .await;
    let done = match done {
        Ok(Ok(done)) => done,
        Ok(Err(f)) => return refused(&f),
        Err(e) => {
            return refused(&Failure {
                code: ErrorCode::Failed,
                message: e.to_string(),
                hint: None,
            });
        }
    };
    let mut value = done.to_json();
    let mut content = Vec::new();
    if include_image {
        match preview(done.path.to_string_lossy().into_owned()).await {
            Ok(p) => {
                value["preview"] =
                    json!({"width": p.width, "height": p.height, "mime_type": p.mime});
                content.push(ContentBlock::image(p.data, p.mime));
            }
            Err(e) => content.push(ContentBlock::text(format!(
                "The preview image could not be attached: {e}"
            ))),
        }
    }
    content.insert(0, ContentBlock::text(value.to_string()));
    let mut result = CallToolResult::success(content);
    result.structured_content = Some(value);
    result
}

fn refused(f: &Failure) -> CallToolResult {
    refusal(&f.reply()).unwrap_or_else(|| failure(&f.message, None))
}

struct Job {
    image: PathBuf,
    ops: String,
    out: PathBuf,
    force: bool,
}

fn job(def: &ToolDef, args: &JsonObject) -> Result<(Job, bool), String> {
    mcp_tools::check_keys(def, args)?;
    let include_image = mcp_tools::include_image(args)?;
    let path = |key: &str| -> Result<PathBuf, String> {
        let text = args
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{key} is required and must be a string"))?;
        let path = PathBuf::from(text);
        if path.is_absolute() {
            Ok(path)
        } else {
            Err(format!("{key} must be an absolute path"))
        }
    };
    let ops = match args.get("ops") {
        Some(ops @ Value::Array(_)) => ops.clone(),
        _ => return Err("ops is required and must be an array".to_owned()),
    };
    let mut document = json!({"version": super::annotate_ops::VERSION, "ops": ops});
    match args.get("background") {
        None | Some(Value::Null) => {}
        Some(background @ Value::Object(_)) => document["background"] = background.clone(),
        Some(_) => return Err("background must be an object".to_owned()),
    }
    let force = match args.get("force") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err("force must be true or false".to_owned()),
    };
    let job = Job {
        image: path("image")?,
        ops: document.to_string(),
        out: path("out")?,
        force,
    };
    Ok((job, include_image))
}
