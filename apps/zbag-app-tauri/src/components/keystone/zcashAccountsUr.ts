/**
 * Custom CBOR decoder for zcash-accounts UR type.
 *
 * This avoids importing @keystonehq/bc-ur-registry-zcash which crashes the app
 * due to CJS/ESM interoperability issues. Instead, we manually decode the CBOR
 * structure following the same pattern as zcashPcztUr.ts for PCZT signing.
 *
 * CBOR structure (from KEYSTONE-TEST-DATA.md):
 * Map {
 *   1: <32 bytes>      // Seed fingerprint (optional, skip)
 *   2: [               // Array of accounts
 *     Tag(49203)       // 0xC033 - wraps UFVK item
 *     Map {
 *       1: <string>    // UFVK encoding
 *       2: <uint>      // Account index (ZIP-32)
 *       3: <string>    // Account label/name
 *     }
 *   ]
 * }
 */

export const ZCASH_ACCOUNTS_UR_TYPE = 'zcash-accounts' as const;
const ZCASH_UFVK_TAG = 49203; // 0xC033

export interface ZcashAccount {
  ufvk: string;
  index?: number;
  label?: string;
}

export interface ZcashAccountsResult {
  seedFingerprint: string | null;
  accounts: ZcashAccount[];
}

/**
 * Read a CBOR head (major type + additional info).
 * Based on zcashPcztUr.ts pattern.
 */
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

/**
 * Read a CBOR unsigned integer.
 */
function readCborUint(bytes: Uint8Array, offset: number): { value: number; offset: number } {
  const head = readCborHead(bytes, offset);
  if (head.major !== 0) throw new Error(`Expected unsigned int (major 0), got major ${head.major}`);
  return { value: head.value, offset: head.offset };
}

/**
 * Read a CBOR text string.
 */
function readCborTextString(bytes: Uint8Array, offset: number): { value: string; offset: number } {
  const head = readCborHead(bytes, offset);
  if (head.major !== 3) throw new Error(`Expected text string (major 3), got major ${head.major}`);
  if (head.indefinite) throw new Error('Indefinite text strings not supported');
  if (head.offset + head.value > bytes.length) throw new Error('Unexpected end of CBOR (text string)');
  const textBytes = bytes.subarray(head.offset, head.offset + head.value);
  return { value: new TextDecoder().decode(textBytes), offset: head.offset + head.value };
}

/**
 * Read a CBOR byte string.
 */
function readCborByteString(bytes: Uint8Array, offset: number): { value: Uint8Array; offset: number } {
  const head = readCborHead(bytes, offset);
  if (head.major !== 2) throw new Error(`Expected byte string (major 2), got major ${head.major}`);
  if (head.indefinite) throw new Error('Indefinite byte strings not supported');
  if (head.offset + head.value > bytes.length) throw new Error('Unexpected end of CBOR (byte string)');
  const byteValue = bytes.subarray(head.offset, head.offset + head.value);
  return { value: byteValue, offset: head.offset + head.value };
}

/**
 * Skip a CBOR item recursively (for items we don't care about).
 */
function skipCborItem(bytes: Uint8Array, offset: number): number {
  const head = readCborHead(bytes, offset);
  let nextOffset = head.offset;

  switch (head.major) {
    case 0: // unsigned int
    case 1: // negative int
    case 7: // simple/float
      return nextOffset;

    case 2: // byte string
    case 3: // text string
      if (head.indefinite) {
        // Skip indefinite chunks until break
        while (bytes[nextOffset] !== 0xff) {
          nextOffset = skipCborItem(bytes, nextOffset);
        }
        return nextOffset + 1; // skip break byte
      }
      return nextOffset + head.value;

    case 4: // array
      if (head.indefinite) {
        while (bytes[nextOffset] !== 0xff) {
          nextOffset = skipCborItem(bytes, nextOffset);
        }
        return nextOffset + 1;
      }
      for (let i = 0; i < head.value; i++) {
        nextOffset = skipCborItem(bytes, nextOffset);
      }
      return nextOffset;

    case 5: // map
      if (head.indefinite) {
        while (bytes[nextOffset] !== 0xff) {
          nextOffset = skipCborItem(bytes, nextOffset); // key
          nextOffset = skipCborItem(bytes, nextOffset); // value
        }
        return nextOffset + 1;
      }
      for (let i = 0; i < head.value; i++) {
        nextOffset = skipCborItem(bytes, nextOffset); // key
        nextOffset = skipCborItem(bytes, nextOffset); // value
      }
      return nextOffset;

    case 6: // tag
      return skipCborItem(bytes, nextOffset); // skip tagged item

    default:
      throw new Error(`Unknown CBOR major type ${head.major}`);
  }
}

/**
 * Parse a single account entry (after reading the tag).
 */
function parseAccountMap(bytes: Uint8Array, offset: number): { account: ZcashAccount; offset: number } {
  const mapHead = readCborHead(bytes, offset);
  if (mapHead.major !== 5) throw new Error(`Expected map (major 5) for account, got major ${mapHead.major}`);
  if (mapHead.indefinite) throw new Error('Indefinite maps not supported for account');

  let nextOffset = mapHead.offset;
  let ufvk: string | undefined;
  let index: number | undefined;
  let label: string | undefined;

  // Iterate map entries (don't assume key order)
  for (let i = 0; i < mapHead.value; i++) {
    const keyResult = readCborUint(bytes, nextOffset);
    const key = keyResult.value;
    nextOffset = keyResult.offset;

    switch (key) {
      case 1: {
        // UFVK string
        const textResult = readCborTextString(bytes, nextOffset);
        ufvk = textResult.value;
        nextOffset = textResult.offset;
        break;
      }
      case 2: {
        // Account index
        const indexResult = readCborUint(bytes, nextOffset);
        index = indexResult.value;
        nextOffset = indexResult.offset;
        break;
      }
      case 3: {
        // Label
        const labelResult = readCborTextString(bytes, nextOffset);
        label = labelResult.value;
        nextOffset = labelResult.offset;
        break;
      }
      default:
        // Unknown key - skip for forward compatibility
        nextOffset = skipCborItem(bytes, nextOffset);
    }
  }

  if (!ufvk) {
    throw new Error('Account entry missing required UFVK (key 1)');
  }

  return {
    account: { ufvk, index, label },
    offset: nextOffset,
  };
}

/**
 * Convert a Uint8Array to a hex string.
 */
function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Decode a zcash-accounts UR CBOR payload.
 *
 * @param cbor The raw CBOR bytes (decoded from UR hex)
 * @returns Object containing seed fingerprint (32-byte hex string) and array of accounts
 */
export function decodeZcashAccountsUrCbor(cbor: Uint8Array): ZcashAccountsResult {
  // 1. Parse outer map
  const outerMapHead = readCborHead(cbor, 0);
  if (outerMapHead.major !== 5) {
    throw new Error(`Invalid zcash-accounts CBOR: expected map (major 5), got major ${outerMapHead.major}`);
  }
  if (outerMapHead.indefinite) {
    throw new Error('Invalid zcash-accounts CBOR: indefinite maps not supported');
  }

  let offset = outerMapHead.offset;
  let seedFingerprint: string | null = null;
  let accountsArray: Uint8Array | null = null;
  let accountsArrayOffset = 0;
  let accountsArrayLength = 0;

  // 2. Iterate outer map entries to find key 1 (seed_fingerprint) and key 2 (accounts array)
  for (let i = 0; i < outerMapHead.value; i++) {
    const keyResult = readCborUint(cbor, offset);
    const key = keyResult.value;
    offset = keyResult.offset;

    if (key === 1) {
      // Found seed fingerprint (32-byte string)
      const byteResult = readCborByteString(cbor, offset);
      if (byteResult.value.length !== 32) {
        throw new Error(`Invalid zcash-accounts CBOR: seed_fingerprint must be 32 bytes, got ${byteResult.value.length}`);
      }
      seedFingerprint = bytesToHex(byteResult.value);
      offset = byteResult.offset;
    } else if (key === 2) {
      // Found accounts array
      const arrayHead = readCborHead(cbor, offset);
      if (arrayHead.major !== 4) {
        throw new Error(`Invalid zcash-accounts CBOR: expected array (major 4) for key 2, got major ${arrayHead.major}`);
      }
      if (arrayHead.indefinite) {
        throw new Error('Invalid zcash-accounts CBOR: indefinite arrays not supported');
      }
      accountsArray = cbor;
      accountsArrayOffset = arrayHead.offset;
      accountsArrayLength = arrayHead.value;
      // Skip past the array for now (we'll parse it below)
      offset = arrayHead.offset;
      for (let j = 0; j < arrayHead.value; j++) {
        offset = skipCborItem(cbor, offset);
      }
    } else {
      // Skip unknown keys
      offset = skipCborItem(cbor, offset);
    }
  }

  if (!accountsArray) {
    throw new Error('Invalid zcash-accounts CBOR: missing accounts array (key 2)');
  }

  // 3. Parse each account entry
  const accounts: ZcashAccount[] = [];
  let entryOffset = accountsArrayOffset;

  for (let i = 0; i < accountsArrayLength; i++) {
    // Read tag (must be 49203)
    const tagHead = readCborHead(accountsArray, entryOffset);
    if (tagHead.major !== 6) {
      throw new Error(`Invalid zcash-accounts CBOR: expected tag (major 6) for account entry, got major ${tagHead.major}`);
    }
    if (tagHead.value !== ZCASH_UFVK_TAG) {
      throw new Error(`Invalid zcash-accounts CBOR: expected tag 49203 (0xC033), got tag ${tagHead.value}`);
    }
    entryOffset = tagHead.offset;

    // Parse the account map
    const accountResult = parseAccountMap(accountsArray, entryOffset);
    accounts.push(accountResult.account);
    entryOffset = accountResult.offset;
  }

  return { seedFingerprint, accounts };
}
