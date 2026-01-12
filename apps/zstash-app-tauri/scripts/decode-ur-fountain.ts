/**
 * Decode UR fountain-encoded QR frames into PCZT binary.
 * Uses the same bc-ur library as the app.
 *
 * Usage (from apps/zkore-app-tauri):
 *   bun run scripts/decode-ur-fountain.ts [input_file]
 *
 * Default input: ../../keystone-qr/video_qr_frames.txt
 * Output: ../../keystone-qr/decoded_pczt.bin (raw PCZT binary)
 */

import { URDecoder } from '@ngraveio/bc-ur';
import { decodeZcashPcztUrCbor, ZCASH_PCZT_UR_TYPE } from '../src/components/signing/zcashPcztUr';
import * as fs from 'fs';
import * as path from 'path';

const keystoneDir = path.resolve(import.meta.dir, '../../../keystone-qr');
const inputFile = process.argv[2] || path.join(keystoneDir, 'video_qr_frames.txt');
const outputFile = path.join(keystoneDir, 'decoded_pczt.bin');
const outputBase64 = path.join(keystoneDir, 'decoded_pczt_base64.txt');

console.log(`Reading QR frames from: ${inputFile}`);

const content = fs.readFileSync(inputFile, 'utf-8');
const lines = content.trim().split('\n').filter(l => l.startsWith('UR:'));

console.log(`Found ${lines.length} UR frames`);

// Create decoder
const decoder = new URDecoder();

let complete = false;
for (let i = 0; i < lines.length; i++) {
  const ur = lines[i];

  try {
    decoder.receivePart(ur);
    const progress = Math.round(decoder.estimatedPercentComplete() * 100);

    if (i % 10 === 0 || progress === 100) {
      console.log(`  Frame ${i + 1}/${lines.length}: ${progress}% complete`);
    }

    if (decoder.isComplete()) {
      complete = true;
      console.log(`\nDecoding complete after ${i + 1} frames!`);
      break;
    }
  } catch (e) {
    console.error(`  Frame ${i + 1} error: ${(e as Error).message}`);
  }
}

if (!complete) {
  console.log(`\nDecoding incomplete. Progress: ${Math.round(decoder.estimatedPercentComplete() * 100)}%`);
  process.exit(1);
}

// Get result
const result = decoder.resultUR();
console.log(`\nUR type: ${result.type}`);
console.log(`Expected UR type: ${ZCASH_PCZT_UR_TYPE}`);
console.log(`CBOR length: ${result.cbor.length} bytes`);

// Decode CBOR - show first bytes
const cborHex = Buffer.from(result.cbor).toString('hex');
console.log(`CBOR hex (first 100 chars): ${cborHex.substring(0, 100)}...`);

// Verify UR type
if (result.type !== ZCASH_PCZT_UR_TYPE) {
  console.error(`\nError: Unexpected UR type. Got '${result.type}', expected '${ZCASH_PCZT_UR_TYPE}'`);
  process.exit(1);
}

// Decode using the same function as the app
try {
  const pcztBytes = decodeZcashPcztUrCbor(result.cbor);

  console.log(`\nPCZT binary length: ${pcztBytes.length} bytes`);

  // Save binary
  fs.writeFileSync(outputFile, pcztBytes);
  console.log(`Saved raw PCZT to: ${outputFile}`);

  // Save base64
  const base64 = Buffer.from(pcztBytes).toString('base64');
  fs.writeFileSync(outputBase64, base64);
  console.log(`Saved base64 PCZT to: ${outputBase64}`);

  // Show first few bytes for debugging
  console.log(`\nFirst 32 bytes (hex): ${Buffer.from(pcztBytes.slice(0, 32)).toString('hex')}`);

  console.log('\nSuccess! The signed PCZT payload has been decoded.');

} catch (e) {
  console.error(`\nError decoding CBOR: ${(e as Error).message}`);
  process.exit(1);
}
