import { expect, test } from 'bun:test';
import { decodeZcashAccountsUrCbor } from '../components/keystone/zcashAccountsUr';

/**
 * Test CBOR hex generated from KEYSTONE-TEST-DATA.md values:
 * - Seed fingerprint: 54d4db6b2cbdd1ecbf5bde6a8187b8fda21eabe6f265775b531683ca43df889c
 * - Account index: 0
 * - Label: "test"
 * - UFVK: mainnet full viewing key (starts with "uview1")
 *
 * CBOR structure: map(2) { 1: bytes(32), 2: [tag(49203) map(3) {1: text, 2: uint, 3: text}] }
 */
const TEST_CBOR_HEX =
  'a201582054d4db6b2cbdd1ecbf5bde6a8187b8fda21eabe6f265775b531683ca43df889c0281d9c033a30179012e7576696577316e6c326e336a3973387177716a326a7873743565766a7a74747475773864653666797930707433353877786a7367337368747361366461666d796b6d64327277396e6376326d3579703571617265636b7a666e38716d73656e6b6d68777a3667387867757834723864726d34737a683771716b3437666c737573783536326c796768367267396633327676637a7276796e347132306567346665303538307a79703363387166346a74386e6e6b6e703332616b636e7136737875716a6b7179377035657063647165666d767071757672666e67663673756e3978397671396a337a68366539357a30797866707a656836763970726c3076793871386632616e6c686a66756e796d65777072717273676172397065716b6575677a6a346b3766386c76746b766337770200036474657374';

const TEST_UFVK =
  'uview1nl2n3j9s8qwqj2jxst5evjztttuw8de6fyy0pt358wxjsg3shtsa6dafmykmd2rw9ncv2m5yp5qareckzfn8qmsenkmhwz6g8xgux4r8drm4szh7qqk47flsusx562lygh6rg9f32vvczrvyn4q20eg4fe0580zyp3c8qf4jt8nnknp32akcnq6sxuqjkqy7p5epcdqefmvpquvrfngf6sun9x9vq9j3zh6e95z0yxfpzeh6v9prl0vy8q8f2anlhjfunymewprqrsgar9peqkeugzj4k7f8lvtkvc7w';

function hexToBytes(hex: string): Uint8Array {
  return new Uint8Array(hex.match(/.{2}/g)!.map((b) => parseInt(b, 16)));
}

test('extracts UFVK from Keystone account export', () => {
  const cbor = hexToBytes(TEST_CBOR_HEX);
  const result = decodeZcashAccountsUrCbor(cbor);

  expect(result.seedFingerprint).toBe('54d4db6b2cbdd1ecbf5bde6a8187b8fda21eabe6f265775b531683ca43df889c');
  expect(result.accounts).toHaveLength(1);
  expect(result.accounts[0].ufvk).toBe(TEST_UFVK);
  expect(result.accounts[0].index).toBe(0);
  expect(result.accounts[0].label).toBe('test');
});

test('rejects invalid CBOR', () => {
  expect(() => decodeZcashAccountsUrCbor(new Uint8Array([0xff]))).toThrow();
});

test('rejects wrong tag', () => {
  // Minimal CBOR: map(1) { 2: [tag(12345) map(1) {1: "test"}] }
  // a1 = map(1), 02 = key 2, 81 = array(1), d9 3039 = tag(12345), a1 = map(1), 01 = key 1, 64 74657374 = text(4) "test"
  const wrongTag = new Uint8Array([0xa1, 0x02, 0x81, 0xd9, 0x30, 0x39, 0xa1, 0x01, 0x64, 0x74, 0x65, 0x73, 0x74]);
  expect(() => decodeZcashAccountsUrCbor(wrongTag)).toThrow(/expected tag 49203/i);
});

test('rejects CBOR without accounts array (key 2)', () => {
  // map(1) { 3: text(4) "test" } - has unknown key 3 but missing key 2 (accounts)
  // a1 = map(1), 03 = key 3, 64 = text(4), 74657374 = "test"
  const noAccountsKey = new Uint8Array([0xa1, 0x03, 0x64, 0x74, 0x65, 0x73, 0x74]);
  expect(() => decodeZcashAccountsUrCbor(noAccountsKey)).toThrow(/missing accounts array/i);
});

test('rejects account entry without UFVK (key 1)', () => {
  // map(1) { 2: [tag(49203) map(1) {2: uint(0)}] }
  // a1 = map(1), 02 = key 2, 81 = array(1), d9 c033 = tag(49203), a1 = map(1), 02 = key 2, 00 = uint(0)
  const noUfvk = new Uint8Array([0xa1, 0x02, 0x81, 0xd9, 0xc0, 0x33, 0xa1, 0x02, 0x00]);
  expect(() => decodeZcashAccountsUrCbor(noUfvk)).toThrow(/missing required UFVK/i);
});
