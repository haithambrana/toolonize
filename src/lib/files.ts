/** Read a user-chosen file as text. FileReader works in every WebView we
 * ship on (and in tests), unlike `Blob.text()` on older engines. */
export function readFileText(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : "");
    reader.onerror = () => reject(reader.error ?? new Error("could not read the file"));
    reader.readAsText(file);
  });
}
