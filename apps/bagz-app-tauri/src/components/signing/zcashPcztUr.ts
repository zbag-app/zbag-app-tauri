export const ZCASH_PCZT_UR_TYPE = 'zcash-pczt' as const;

function encodeCborMajorLength(major: number, length: number): Uint8Array {
  if (!Number.isSafeInteger(length) || length < 0) throw new Error('Invalid CBOR length');

  if (length < 24) return Uint8Array.of((major << 5) | length);

  if (length < 0x100) return Uint8Array.of((major << 5) | 24, length);

  if (length < 0x10000) return Uint8Array.of((major << 5) | 25, length >> 8, length & 0xff);

  if (length < 0x1_0000_0000) {
    return Uint8Array.of(
      (major << 5) | 26,
      (length >>> 24) & 0xff,
      (length >>> 16) & 0xff,
      (length >>> 8) & 0xff,
      length & 0xff
    );
  }

  const n = BigInt(length);
  const hi = Number((n >> 32n) & 0xffff_ffffn);
  const lo = Number(n & 0xffff_ffffn);
  return Uint8Array.of(
    (major << 5) | 27,
    (hi >>> 24) & 0xff,
    (hi >>> 16) & 0xff,
    (hi >>> 8) & 0xff,
    hi & 0xff,
    (lo >>> 24) & 0xff,
    (lo >>> 16) & 0xff,
    (lo >>> 8) & 0xff,
    lo & 0xff
  );
}

function concatBytes(chunks: Uint8Array[]): Uint8Array {
  const totalLength = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function readCborHead(bytes: Uint8Array, offset: number): {
  major: number;
  value: number;
  offset: number;
  indefinite: boolean;
} {
  if (offset >= bytes.length) throw new Error('Unexpected end of CBOR');

  const initial = bytes[offset];
  const major = initial >> 5;
  const ai = initial & 0x1f;
  let nextOffset = offset + 1;

  if (ai < 24) {
    return { major, value: ai, offset: nextOffset, indefinite: false };
  }

  if (ai === 24) {
    if (nextOffset + 1 > bytes.length) throw new Error('Unexpected end of CBOR');
    const value = bytes[nextOffset];
    nextOffset += 1;
    return { major, value, offset: nextOffset, indefinite: false };
  }

  if (ai === 25) {
    if (nextOffset + 2 > bytes.length) throw new Error('Unexpected end of CBOR');
    const value = (bytes[nextOffset] << 8) | bytes[nextOffset + 1];
    nextOffset += 2;
    return { major, value, offset: nextOffset, indefinite: false };
  }

  if (ai === 26) {
    if (nextOffset + 4 > bytes.length) throw new Error('Unexpected end of CBOR');
    const value =
      (bytes[nextOffset] * 0x1_0000_00) +
      (bytes[nextOffset + 1] << 16) +
      (bytes[nextOffset + 2] << 8) +
      bytes[nextOffset + 3];
    nextOffset += 4;
    return { major, value, offset: nextOffset, indefinite: false };
  }

  if (ai === 27) {
    if (nextOffset + 8 > bytes.length) throw new Error('Unexpected end of CBOR');
    let n = 0n;
    for (let i = 0; i < 8; i += 1) {
      n = (n << 8n) | BigInt(bytes[nextOffset + i]);
    }
    nextOffset += 8;
    if (n > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new Error('CBOR length too large');
    }
    return { major, value: Number(n), offset: nextOffset, indefinite: false };
  }

  if (ai === 31) {
    return { major, value: 0, offset: nextOffset, indefinite: true };
  }

  throw new Error('Unsupported CBOR additional info');
}

export function encodeZcashPcztUrCbor(pcztBytes: Uint8Array): Uint8Array {
  const mapHeader = encodeCborMajorLength(5, 1);
  const keyHeader = encodeCborMajorLength(0, 1);
  const valueHeader = encodeCborMajorLength(2, pcztBytes.length);

  const out = new Uint8Array(mapHeader.length + keyHeader.length + valueHeader.length + pcztBytes.length);
  let offset = 0;
  out.set(mapHeader, offset);
  offset += mapHeader.length;
  out.set(keyHeader, offset);
  offset += keyHeader.length;
  out.set(valueHeader, offset);
  offset += valueHeader.length;
  out.set(pcztBytes, offset);
  return out;
}

export function decodeZcashPcztUrCbor(cbor: Uint8Array): Uint8Array {
  let offset = 0;

  const map = readCborHead(cbor, offset);
  offset = map.offset;
  if (map.major !== 5 || map.indefinite || map.value !== 1) {
    throw new Error('Invalid zcash-pczt CBOR: expected map(1)');
  }

  const key = readCborHead(cbor, offset);
  offset = key.offset;
  if (key.major !== 0 || key.indefinite || key.value !== 1) {
    throw new Error('Invalid zcash-pczt CBOR: expected key 1');
  }

  const value = readCborHead(cbor, offset);
  offset = value.offset;
  if (value.major !== 2) {
    throw new Error('Invalid zcash-pczt CBOR: expected byte string');
  }

  if (!value.indefinite) {
    if (offset + value.value > cbor.length) throw new Error('Invalid zcash-pczt CBOR: truncated data');
    const data = cbor.subarray(offset, offset + value.value);
    offset += value.value;
    if (offset !== cbor.length) throw new Error('Invalid zcash-pczt CBOR: trailing bytes');
    return data;
  }

  const chunks: Uint8Array[] = [];
  while (offset < cbor.length) {
    if (cbor[offset] === 0xff) {
      offset += 1;
      break;
    }
    const chunkHead = readCborHead(cbor, offset);
    offset = chunkHead.offset;
    if (chunkHead.major !== 2 || chunkHead.indefinite) {
      throw new Error('Invalid zcash-pczt CBOR: invalid chunk');
    }
    if (offset + chunkHead.value > cbor.length) throw new Error('Invalid zcash-pczt CBOR: truncated chunk');
    chunks.push(cbor.subarray(offset, offset + chunkHead.value));
    offset += chunkHead.value;
  }

  if (offset !== cbor.length) throw new Error('Invalid zcash-pczt CBOR: trailing bytes');
  return concatBytes(chunks);
}

