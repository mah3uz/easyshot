export const mcpTools = ['list_outputs', 'list_windows', 'capture', 'ocr', 'record_start', 'record_stop', 'export', 'annotate'] as const

export const mcpClients = {
  'Claude Code': {
    note: 'Add <code>--scope project</code> to write <code>.mcp.json</code> for a project.',
    code: 'claude mcp add easyshot -- easyshot agent mcp',
    prompt: true,
  },
  Codex: {
    note: 'In <code>~/.codex/config.toml</code>. <code>codex mcp add easyshot -- easyshot agent mcp</code> writes the same.',
    code: '[mcp_servers.easyshot]\ncommand = "easyshot"\nargs = ["agent", "mcp"]',
    prompt: false,
  },
  'Gemini CLI': {
    note: 'Writes <code>~/.gemini/settings.json</code>, or add the server there by hand.',
    code: 'gemini mcp add easyshot easyshot agent mcp',
    prompt: true,
  },
  Cursor: {
    note: 'In <code>~/.cursor/mcp.json</code>, or <code>.cursor/mcp.json</code> in a project.',
    code: '{ "mcpServers": { "easyshot": { "type": "stdio", "command": "easyshot", "args": ["agent", "mcp"] } } }',
    prompt: false,
  },
} satisfies Record<string, { note: string; code: string; prompt: boolean }>

export type McpClient = keyof typeof mcpClients
