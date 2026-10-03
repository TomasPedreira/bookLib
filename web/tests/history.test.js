import test from 'node:test';
import assert from 'node:assert/strict';
import {readingDays} from '../history.js';

test('history groups each day by its final saved position and matches daily statistics', () => {
  const progress = [
    {id: 2, value: 12, recorded_at: '2026-10-01'},
    {id: 1, value: 10, recorded_at: '2026-10-01 23:00:00'},
    {id: 3, value: 20, recorded_at: '2026-10-02'},
    {id: 4, value: 18, recorded_at: '2026-10-03'},
    {id: 5, value: 25, recorded_at: '2026-10-04'},
  ];
  const before = structuredClone(progress);
  const days = readingDays(progress);
  assert.deepEqual(days.map(day => [day.day, day.id, day.pages, day.previous, day.value]), [
    ['2026-10-04', 5, 7, 18, 25],
    ['2026-10-03', 4, 0, 20, 18],
    ['2026-10-02', 3, 8, 12, 20],
    ['2026-10-01', 2, 12, 0, 12],
  ]);
  assert.deepEqual(progress, before);
});
