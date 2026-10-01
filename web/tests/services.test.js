import test from 'node:test';
import assert from 'node:assert/strict';
import { createServices } from '../services.js';

test('library operations send structured payloads directly to native commands', async () => {
  const calls = [];
  const result = { id: 7 };
  const { library } = createServices(async (name, args) => { calls.push({ name, args }); return result; });
  const book = { title: 'Book', authors: 'Author', page_count: 200 };
  assert.equal(await library.create(book), result);
  await library.update(7, book);
  await library.start(7, 'pages');
  await library.addProgress(3, { value: 20, recorded_at: '2026-10-01' });
  await library.rate(7, null);
  assert.deepEqual(calls, [
    { name: 'create_book', args: { b: book } },
    { name: 'update_book', args: { id: 7, b: book } },
    { name: 'start_reading', args: { id: 7, input: { unit: 'pages' } } },
    { name: 'add_progress', args: { id: 3, input: { value: 20, recorded_at: '2026-10-01' } } },
    { name: 'update_rating', args: { id: 7, input: { rating: null } } },
  ]);
});

test('catalog queries retain filters and edition pagination', async () => {
  const calls = [];
  const { catalog } = createServices(async (name, args) => { calls.push({ name, args }); return { items: [] }; });
  await catalog.recommendations({ genres: 'fantasy', languages: 'por', mode: 'home' });
  await catalog.editions('OL123W', 50);
  assert.deepEqual(calls, [
    { name: 'recommendations', args: { query: { genres: 'fantasy', languages: 'por', mode: 'home' } } },
    { name: 'editions', args: { id: 'OL123W', query: { offset: 50 } } },
  ]);
});

test('native validation errors remain readable to the user', async () => {
  const { library } = createServices(async () => { throw { code: 400, message: 'Indica um valor entre 0 e 100' }; });
  await assert.rejects(library.addProgress(3, { value: 101 }), /Enter a value between 0 and 100/);
});
