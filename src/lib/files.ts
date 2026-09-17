// Sending files. The frontend reads the file in chunks and hands each one to
// Rust as base64, because Android's IPC carries JSON only. Rust seals every
// chunk and relays it to the target devices as it arrives.
import { api } from "./api";
import type { SendResult, SendTarget } from "./types";

/** Keep in sync with MAX_FILE_BYTES and FILE_CHUNK_BYTES in src-tauri/src/protocol.rs. */
export const MAX_FILE_BYTES = 100 * 1024 * 1024;
const CHUNK_BYTES = 512 * 1024;

/**
 * Sends one file to `target`. `onProgress` gets the fraction sent (0 to 1).
 * Rejects when no device accepted the file or the transfer broke off; per-device
 * failures at the end come back in the results.
 */
export async function sendFile(
  target: SendTarget,
  file: File,
  onProgress: (fraction: number) => void,
): Promise<SendResult[]> {
  const id = await api.sendFileBegin(target, file.name, file.size, file.type);
  try {
    for (let offset = 0; offset < file.size; offset += CHUNK_BYTES) {
      const bytes = new Uint8Array(await file.slice(offset, offset + CHUNK_BYTES).arrayBuffer());
      const sent = await api.sendFileChunk(id, toBase64(bytes));
      onProgress(sent / file.size);
    }
    return await api.sendFileEnd(id);
  } catch (err) {
    void api.sendFileAbort(id).catch(() => {});
    throw err;
  }
}

const ALPHABET = new TextEncoder().encode(
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
);
const PAD = 61; // "="

/**
 * Base64 of `bytes`. Encoding dominates the send loop on phones, so this uses
 * the native encoder where the WebView has one and a table-driven encoder
 * (about 20 times faster than btoa over a char-by-char string) elsewhere.
 */
function toBase64(bytes: Uint8Array): string {
  const native = (bytes as Uint8Array & { toBase64?: () => string }).toBase64;
  if (native) return native.call(bytes);

  const out = new Uint8Array(Math.ceil(bytes.length / 3) * 4);
  let o = 0;
  let i = 0;
  for (; i + 2 < bytes.length; i += 3) {
    const n = (bytes[i] << 16) | (bytes[i + 1] << 8) | bytes[i + 2];
    out[o++] = ALPHABET[n >> 18];
    out[o++] = ALPHABET[(n >> 12) & 63];
    out[o++] = ALPHABET[(n >> 6) & 63];
    out[o++] = ALPHABET[n & 63];
  }
  const rest = bytes.length - i;
  if (rest > 0) {
    const n = (bytes[i] << 16) | (rest === 2 ? bytes[i + 1] << 8 : 0);
    out[o++] = ALPHABET[n >> 18];
    out[o++] = ALPHABET[(n >> 12) & 63];
    out[o++] = rest === 2 ? ALPHABET[(n >> 6) & 63] : PAD;
    out[o++] = PAD;
  }
  return new TextDecoder().decode(out);
}
