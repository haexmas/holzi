// How the transcript shows the text of a tool call and its result (pure, so the Node check scripts
// can load it). The model and the tools exchange JSON on one line; a person reads it indented.

/** `text` indented when it is a JSON object or array, otherwise unchanged (markers, plain text). */
export function prettyToolText(text: string): string {
  const trimmed = text.trim()
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) return text
  try {
    return JSON.stringify(JSON.parse(trimmed), null, 2)
  } catch {
    return text
  }
}
