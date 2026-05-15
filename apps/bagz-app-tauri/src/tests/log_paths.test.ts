import { expect, test } from 'bun:test';
import { resolveLogRevealPath } from '../lib/logPaths';

test('resolveLogRevealPath prefers current log file when present', () => {
  expect(
    resolveLogRevealPath({
      current_log_file: '/tmp/bagz/current.log',
      log_directory: '/tmp/bagz',
    })
  ).toBe('/tmp/bagz/current.log');
});

test('resolveLogRevealPath falls back to log directory', () => {
  expect(
    resolveLogRevealPath({
      current_log_file: '   ',
      log_directory: '/tmp/bagz',
    })
  ).toBe('/tmp/bagz');
});

test('resolveLogRevealPath trims surrounding whitespace', () => {
  expect(
    resolveLogRevealPath({
      current_log_file: '  /tmp/bagz/current.log  ',
      log_directory: '/tmp/bagz',
    })
  ).toBe('/tmp/bagz/current.log');
});

test('resolveLogRevealPath returns null when both paths are empty', () => {
  expect(
    resolveLogRevealPath({
      current_log_file: ' ',
      log_directory: '',
    })
  ).toBeNull();
});
