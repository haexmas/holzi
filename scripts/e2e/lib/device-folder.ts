// The folder a device's material goes into (contracts/failure-material.md): both address segments are
// percent-encoded and written with their length, so a "/" or "-" inside a name can neither make a path
// separator nor make two devices share a folder.

function segment(value: string): string {
  const encoded = encodeURIComponent(value).replaceAll('%', '_')
  return `${encoded.length}-${encoded}`
}

/** `anna/laptop` becomes `4-anna-6-laptop`. */
export function deviceFolder(user: string, device: string): string {
  return `${segment(user)}-${segment(device)}`
}
