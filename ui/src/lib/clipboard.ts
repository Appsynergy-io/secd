/** Write to the clipboard. The payload is never logged. */

export async function copyText(text: string): Promise<boolean> {
  const clip = globalThis.navigator?.clipboard;
  if (!clip) {
    return false;
  }
  try {
    await clip.writeText(text);
    return true;
  } catch {
    return false;
  }
}
