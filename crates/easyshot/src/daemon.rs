#[derive(Debug, Default)]
pub struct DaemonArgs;

pub fn run(_args: DaemonArgs) -> anyhow::Result<()> {
    anyhow::bail!("the daemon is not implemented yet")
}
