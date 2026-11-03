use super::AgentCall;
use super::mcp_image::preview;
use super::mcp_tools::{self, ToolDef};
use super::run::ask_via;
use crate::cli::{Cli, Request};
use crate::run::{EXIT_ERROR, EXIT_OK, Env, launch_with};
use clap::Parser;
use es_desktop::dbus::Endpoint;
use es_protocol::agent::{AgentReply, ErrorCode};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use serde_json::{Value, json};
use std::io::Write;
use std::time::Duration;

const INSTRUCTIONS: &str = "Screenshots, OCR and screen recordings of the user's desktop through \
    the easyshot daemon. The user must have turned agent access on and may be asked to allow each \
    session; if a call fails with access_off or access_denied, tell the user and do not retry on \
    your own. Call list_outputs or list_windows to find targets.";

#[derive(Clone)]
pub(super) struct Server {
    endpoint: Endpoint,
    spawn_daemon: bool,
    wait: Duration,
}

impl Server {
    pub(super) fn new(endpoint: Endpoint, spawn_daemon: bool, wait: Duration) -> Server {
        Server {
            endpoint,
            spawn_daemon,
            wait,
        }
    }

    async fn call(&self, def: &ToolDef, args: JsonObject) -> CallToolResult {
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
        let Some(def) = mcp_tools::find(&request.name) else {
            return Err(McpError::invalid_params(
                format!("unknown tool {}", request.name),
                None,
            ));
        };
        let args = request.arguments.unwrap_or_default();
        Ok(self.call(def, args).await.into())
    }
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

fn failure(message: &str, structured: Option<Value>) -> CallToolResult {
    let mut result = CallToolResult::error(vec![ContentBlock::text(message)]);
    result.structured_content = structured;
    result
}

fn hint(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::AccessOff => {
            " Ask the user to turn agent access on in Settings > Agents, \
             or to run `easyshot agent enable` themselves."
        }
        ErrorCode::AccessDenied => {
            " The user denied this request. Do not retry without asking them."
        }
        ErrorCode::NoDaemon => " Ask the user to start the easyshot daemon.",
        _ => "",
    }
}

async fn result(def: &ToolDef, reply: &AgentReply, include_image: bool) -> CallToolResult {
    let mut value = serde_json::to_value(reply).unwrap_or(Value::Null);
    if let Some(error) = &reply.error {
        let message = format!(
            "{}: {}.{}",
            error.code.as_str(),
            error.message.trim_end_matches('.'),
            hint(error.code)
        );
        return failure(&message, Some(value));
    }
    if !reply.ok {
        return failure("failed: the daemon reported a failure", Some(value));
    }
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
