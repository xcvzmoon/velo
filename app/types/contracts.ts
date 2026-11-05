export type DiagnosticInput = { label: string };
export type DiagnosticReport = { engine: string; version: string };
export type CommandError = { code: string; message: string; field: string | null };
