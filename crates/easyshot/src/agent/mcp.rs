use super::AgentCall;
use super::mcp_image::preview;
use super::mcp_tools::{self, ToolDef};
use super::run::ask_via;
use crate::cli::{Cli, Request};
use crate::run::{EXIT_ERROR, EXIT_OK, Env, launch_with};
use clap::Parser;
use es_desktop::dbus::Endpoint;
use es_protocol::agent::AgentReply;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use serde_json::{Value, json};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const INSTRUCTIONS: &str = "Screenshots, OCR and screen recordings of the user's real desktop through \
    the easyshot daemon. Call list_windows or list_outputs first, and prefer a window target: it \
    is captured, read and recorded without focusing it, and works on covered windows and windows \
    on other workspaces. Use ocr for exact text, which is cheaper than an image, and set temp on \
    capture for repeated checks so the user's gallery stays clean. Pass any error hint to the user \
    as it is. Turning access on, consent and scope are the user's decisions: on access_off or \
    access_denied ask the user and do not retry on your own.";

const UPDATED: &str = "easyshot was updated while this MCP server was running, so these tools \
    are the old version's. Ask the user to reconnect the easyshot MCP server or restart the agent \
    session to get the new tools.";

type ExeLink = Arc<dyn Fn() -> std::io::Result<PathBuf> + Send + Sync>;

#[derive(Clone)]
pub(super) struct Server {
    endpoint: Endpoint,
    spawn_daemon: bool,
    wait: Duration,
    exe: ExeLink,
}

impl Server {
    pub(super) fn new(endpoint: Endpoint, spawn_daemon: bool, wait: Duration) -> Server {
        Server {
            endpoint,
            spawn_daemon,
            wait,
            exe: Arc::new(|| std::fs::read_link("/proc/self/exe")),
        }
    }

    #[cfg(test)]
    pub(super) fn with_exe(mut self, exe: ExeLink) -> Server {
        self.exe = exe;
        self
    }

    async fn call(&self, def: &ToolDef, args: JsonObject) -> CallToolResult {
        if def.name == mcp_tools::ANNOTATE {
            return super::mcp_annotate::call(def, &args).await;
        }
        let (call, include_image) = match prepare(def, &args) {
            Ok(prepared) => prepared,
            Err(message) => return failure(&message, None),
        };
        let reply = ask_via(
            &call,
            &self.endpoint,
            launch_with(self.spawn_daemon, self.wait),
            std::future::pending(),
        )
        .await;
        result(def, &reply, include_image).await
    }
}

pub(super) async fn run_on<T, E, A>(server: Server, transport: T) -> Result<(), String>
where
    T: IntoTransport<RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let running = server.serve(transport).await.map_err(|e| e.to_string())?;
    running
        .waiting()
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn serve(env: &Env, err: &mut dyn Write) -> i32 {
    let server = Server::new(env.endpoint.clone(), env.spawn_daemon, env.wait);
    let outcome = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())
        .and_then(|rt| rt.block_on(run_on(server, rmcp::transport::stdio())));
    match outcome {
        Ok(()) => EXIT_OK,
        Err(e) => {
            let _ = writeln!(err, "easyshot: {e}");
            EXIT_ERROR
        }
    }
}

impl ServerHandler for Server {
    #[allow(deprecated)]
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("easyshot", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(mcp_tools::tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let stale = replaced((self.exe)());
        let Some(def) = mcp_tools::find(&request.name) else {
            let mut message = format!("unknown tool {}", request.name);
            if stale {
                message = format!("{message}. {UPDATED}");
            }
            return Err(McpError::invalid_params(message, None));
        };
        let args = request.arguments.unwrap_or_default();
        let mut result = self.call(def, args).await;
        if stale {
            result.content.push(ContentBlock::text(UPDATED));
        }
        Ok(result.into())
    }
}

pub(super) fn replaced(exe: std::io::Result<PathBuf>) -> bool {
    exe.is_ok_and(|path| path.as_os_str().as_bytes().ends_with(b" (deleted)"))
}

pub(super) fn prepare(def: &ToolDef, args: &JsonObject) -> Result<(AgentCall, bool), String> {
    mcp_tools::check_keys(def, args)?;
    let include_image = mcp_tools::include_image(args)?;
    let argv = (def.argv)(args)?;
    let cli = Cli::try_parse_from(
        ["easyshot", "agent"]
            .into_iter()
            .map(str::to_owned)
            .chain(argv),
    )
    .map_err(|e| first_line(&e))?;
    match cli.request().map_err(|e| e.to_string())? {
        Request::Agent(call) => Ok((call, include_image)),
        _ => Err("not an agent command".to_owned()),
    }
}

fn first_line(e: &clap::Error) -> String {
    let rendered = e.render().to_string();
    rendered
        .lines()
        .next()
        .unwrap_or_default()
        .trim_start_matches("error: ")
        .to_owned()
}

pub(super) fn failure(message: &str, structured: Option<Value>) -> CallToolResult {
    let mut result = CallToolResult::error(vec![ContentBlock::text(message)]);
    result.structured_content = structured;
    result
}

pub(super) fn refusal(reply: &AgentReply) -> Option<CallToolResult> {
    let value = serde_json::to_value(reply).unwrap_or(Value::Null);
    if let Some(error) = &reply.error {
        let mut message = format!(
            "{}: {}.",
            error.code.as_str(),
            error.message.trim_end_matches('.')
        );
        if let Some(hint) = &error.hint {
            message.push(' ');
            message.push_str(hint);
        }
        return Some(failure(&message, Some(value)));
    }
    (!reply.ok).then(|| failure("failed: the daemon reported a failure", Some(value)))
}

async fn result(def: &ToolDef, reply: &AgentReply, include_image: bool) -> CallToolResult {
    if let Some(refused) = refusal(reply) {
        return refused;
    }
    let mut value = serde_json::to_value(reply).unwrap_or(Value::Null);
    let mut note = None;
    let mut image = None;
    if let Some(shot) = reply
        .capture
        .as_ref()
        .filter(|_| def.name == mcp_tools::CAPTURE && include_image)
    {
        match preview(shot.path.clone()).await {
            Ok(p) => {
                value["preview"] =
                    json!({"width": p.width, "height": p.height, "mime_type": p.mime});
                image = Some(ContentBlock::image(p.data, p.mime));
            }
            Err(e) => note = Some(format!("The preview image could not be attached: {e}")),
        }
    }
    let mut content = vec![ContentBlock::text(value.to_string())];
    content.extend(image);
    content.extend(note.map(ContentBlock::text));
    let mut result = CallToolResult::success(content);
    result.structured_content = Some(value);
    result
}
