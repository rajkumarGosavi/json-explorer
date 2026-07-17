/** Write text to the system clipboard, swallowing failures (clipboard may be
 * unavailable in some webview contexts — there's nothing sensible to do). */
export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Clipboard unavailable — ignore.
  }
}
