mod run;
#[cfg(test)]
mod tests;

pub use run::{run, usage_error};

use clap::{Args, Subcommand, ValueEnum};
use es_protocol::AgentRoute;

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
    pub fn call(&self) -> AgentCall {
        let route = match self.command {
            AgentCommand::List {
                what: ListWhat::Outputs,
            } => AgentRoute::ListOutputs,
            AgentCommand::List {
                what: ListWhat::Windows,
            } => AgentRoute::ListWindows,
        };
        AgentCall {
            route,
            json: self.json,
            wait: true,
        }
    }
}
