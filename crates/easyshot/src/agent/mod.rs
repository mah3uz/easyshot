mod access;
mod capture;
mod export;
mod ocr;
mod record;
mod run;
mod skill;
mod skill_install;
#[cfg(test)]
mod tests;

pub use capture::CaptureArgs;
pub use export::ExportArgs;
pub use ocr::OcrArgs;
pub use record::RecordCommand;
pub use run::{run, usage_error};
pub use skill::{SkillCall, SkillCommand, run_skill};

use clap::{Args, Subcommand, ValueEnum};
use es_protocol::AgentRoute;
use es_protocol::agent::ManageRoute;

#[derive(Args, Debug)]
pub struct AgentArgs {
    /// Print exactly one JSON object on standard output, errors included
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Subcommand, Debug)]
pub enum AgentCommand {
    /// List the outputs or the open windows
    List { what: ListWhat },
    /// Take a screenshot of one target and print where it was saved
    Capture(CaptureArgs),
    /// Turn agent access on
    Enable,
    /// Turn agent access off and end every grant
    Disable,
    /// Show whether agent access is on, the grant in force and the limits
    Status,
    /// Read or change an agent access setting
    #[command(subcommand)]
    Config(access::ConfigCommand),
    /// Re-encode a recording as MP4, WebM or GIF with the trim viewer's export options
    Export(ExportArgs),
    /// Read the text and QR codes on a target or in an image file
    Ocr(OcrArgs),
    /// Record the screen: start, stop, status and discard
    #[command(subcommand)]
    Record(RecordCommand),
    /// Install the easyshot skill for AI agents, or show it
    #[command(subcommand)]
    Skill(SkillCommand),
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListWhat {
    Outputs,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCall {
    pub route: AgentRoute,
    pub json: bool,
    /// Wait for the job's `JobFinished` instead of returning once the daemon accepts it.
    pub wait: bool,
}

impl AgentArgs {
    pub fn skill(&self) -> Option<SkillCall> {
        match &self.command {
            AgentCommand::Skill(s) => Some(s.call(self.json)),
            _ => None,
        }
    }

    pub fn call(&self) -> Option<AgentCall> {
        let (route, wait) = match &self.command {
            AgentCommand::List {
                what: ListWhat::Outputs,
            } => (AgentRoute::ListOutputs, true),
            AgentCommand::List {
                what: ListWhat::Windows,
            } => (AgentRoute::ListWindows, true),
            AgentCommand::Capture(args) => (AgentRoute::Capture(args.spec()), args.wait()),
            AgentCommand::Enable => (AgentRoute::Manage(ManageRoute::Enable), true),
            AgentCommand::Disable => (AgentRoute::Manage(ManageRoute::Disable), true),
            AgentCommand::Status => (AgentRoute::Manage(ManageRoute::Status), true),
            AgentCommand::Config(c) => (AgentRoute::Manage(c.route()), true),
            AgentCommand::Export(args) => (AgentRoute::Export(args.spec()), args.wait()),
            AgentCommand::Ocr(args) => (AgentRoute::Ocr(args.spec()), true),
            AgentCommand::Record(c) => (AgentRoute::Record(c.route()), true),
            AgentCommand::Skill(_) => return None,
        };
        Some(AgentCall {
            route,
            json: self.json,
            wait,
        })
    }
}
