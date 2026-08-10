type ZxingResult = {
  getResultMetadata(): Map<number, unknown>;
};

const BYTE_SEGMENTS = 2;

export function payloadFromResult(result: ZxingResult): Uint8Array | undefined {
  const segments = result.getResultMetadata().get(BYTE_SEGMENTS);
  if (!Array.isArray(segments) || !segments.length) return undefined;
  const bytes = segments.filter((segment): segment is Uint8Array => segment instanceof Uint8Array);
  if (!bytes.length) return undefined;
  if (bytes.length === 1) return bytes[0];
  const payload = new Uint8Array(bytes.reduce((length, segment) => length + segment.length, 0));
  let offset = 0;
  for (const segment of bytes) {
    payload.set(segment, offset);
    offset += segment.length;
  }
  return payload;
}
