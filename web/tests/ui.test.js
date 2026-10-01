import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';

test('UI adds a book, records progress, handles back and saves a native backup', async () => {
  const dom = new JSDOM(await readFile(new URL('../index.html', import.meta.url), 'utf8'), { url: 'http://localhost/' });
  const { window } = dom;
  for (const name of ['window', 'document', 'location', 'history', 'localStorage', 'FormData']) {
    globalThis[name] = name === 'window' ? window : window[name];
  }
  window.scrollTo = () => {};
  window.HTMLElement.prototype.scrollTo = () => {};
  window.HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  window.HTMLDialogElement.prototype.close = function () { this.open = false; this.dispatchEvent(new window.Event('close')); };
  const calls = [], books = [], sessions = new Map(), callbacks = new Map();
  let serial = 0, backChannel;
  window.__TAURI_INTERNALS__ = {
    transformCallback: callback => { callbacks.set(++serial, callback); return serial; },
    unregisterCallback: id => callbacks.delete(id),
    invoke: async (name, args = {}, options = {}) => {
      calls.push({ name, args, options });
      if (name === 'plugin:app|register_listener') { backChannel = args.handler.id; return; }
      if (name === 'plugin:dialog|save') return 'content://documents/backup.json';
      if (name === 'plugin:dialog|open') return 'content://documents/backup.json';
      if (name === 'plugin:dialog|message') return 'Ok';
      if (name === 'plugin:fs|write_text_file') return;
      if (name === 'plugin:fs|read_text_file') {
        const saved = calls.find(c => c.name === 'plugin:fs|write_text_file');
        return saved.args;
      }
      if (name === 'import_data') return { ok: true, books: args.data.books.length };
      if (name === 'plugin:opener|open_url') return;
      if (name === 'list_books') return books.map(book => ({ book, reading: sessions.get(book.id)?.reading || null }));
      if (name === 'stats') return { total: books.length, want: books.length, reading: 0, paused: 0, completed: 0, abandoned: 0, reading_sessions: 0, pages_read: 0, daily_pages: [] };
      if (name === 'recommendations') return { items: [] };
      if (name === 'create_book') {
        const book = { ...args.b, id: books.length + 1, status: 'want', created_at: '2026-10-01', updated_at: '2026-10-01' };
        books.push(book); return book;
      }
      if (name === 'get_book') return { book: books.find(b => b.id === args.id), readings: sessions.has(args.id) ? [sessions.get(args.id)] : [] };
      if (name === 'start_reading') {
        const reading = { id: 1, book_id: args.id, unit: args.input.unit, status: 'reading', current_value: 0 };
        sessions.set(args.id, { reading, progress: [] });
        books.find(b => b.id === args.id).status = 'reading'; return reading;
      }
      if (name === 'add_progress') {
        const session = [...sessions.values()].find(s => s.reading.id === args.id);
        session.reading.current_value = args.input.value;
        const entry = { id: 1, reading_id: args.id, ...args.input };
        session.progress.push(entry); return entry;
      }
      if (name === 'export_data') return { version: 2, books, readings: [...sessions.values()].map(s => s.reading), progress_entries: [...sessions.values()].flatMap(s => s.progress) };
      throw new Error(`Unexpected native command: ${name}`);
    },
  };
  const waitFor = async predicate => {
    for (let i = 0; i < 50; i++) {
      if (predicate()) return;
      await new Promise(resolve => setTimeout(resolve, 5));
    }
    assert.fail(`UI did not reach the expected state: ${$('#toast').textContent}; ${calls.slice(-4).map(c => c.name).join(', ')}`);
  };
  const $ = selector => window.document.querySelector(selector);
  const submit = selector => $(selector).dispatchEvent(new window.Event('submit', { bubbles: true, cancelable: true }));
  try {
    await import('../app.js');
    await waitFor(() => calls.some(c => c.name === 'stats'));
    $('#manual-add').click();
    $('#book-form').elements.title.value = 'Android UI test';
    $('#book-form').elements.page_count.value = '200';
    submit('#book-form');
    await waitFor(() => $('#detail-dialog').open);
    $('[data-action="start"]').click();
    await waitFor(() => $('[data-action="progress"]'));
    $('[data-action="progress"]').click();
    $('#progress-form').elements.value.value = '42';
    submit('#progress-form');
    await waitFor(() => $('#detail-dialog').open && $('#detail-content').textContent.includes('42 / 200 pages'));
    assert.equal($('#page-1').value, '42');

    // Deliver the actual Tauri channel envelope used by Android's back event.
    callbacks.get(backChannel)({ index: 0, message: { canGoBack: false } });
    assert.equal($('#detail-dialog').open, false);
    $('#export-button').click();
    await waitFor(() => calls.some(c => c.name === 'plugin:fs|write_text_file'));
    const saved = calls.find(c => c.name === 'plugin:fs|write_text_file');
    assert.equal(decodeURIComponent(saved.options.headers.path), 'content://documents/backup.json');
    const backup = JSON.parse(new TextDecoder().decode(saved.args));
    assert.equal(backup.version, 2);
    assert.equal(backup.progress_entries[0].value, 42);
    $('#import-button').click();
    await waitFor(() => calls.some(c => c.name === 'import_data'));
    assert.deepEqual(calls.find(c => c.name === 'import_data').args.data, backup);
    assert.ok(calls.some(c => c.name === 'plugin:dialog|message'));
    $('.source-link').click();
    await waitFor(() => calls.some(c => c.name === 'plugin:opener|open_url'));
    assert.equal(calls.find(c => c.name === 'plugin:opener|open_url').args.url, 'https://openlibrary.org/');
    assert.equal(calls.some(c => c.name.includes('/api/')), false);
  } finally { window.close(); }
});
