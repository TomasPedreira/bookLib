import test from 'node:test';
import assert from 'node:assert/strict';
import { validateBackup } from '../backup.js';

test('accepts current and legacy backups without changing their contents', () => {
  for (const version of [1, 2]) {
    const backup = { version, books: [{ id: 1 }], readings: [], progress_entries: [] };
    assert.equal(validateBackup(backup), backup);
  }
});

test('rejects unknown versions and incomplete files before requesting a restore', () => {
  for (const backup of [null, {}, { version: 3, books: [], readings: [], progress_entries: [] },
    { version: 2, books: [], readings: [] }, { version: 2, books: {}, readings: [], progress_entries: [] }]) {
    assert.throws(() => validateBackup(backup), /Invalid backup file/);
  }
});
