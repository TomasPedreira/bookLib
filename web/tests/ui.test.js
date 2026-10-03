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
  const activityViewport = new window.EventTarget();
  activityViewport.matches = true;
  window.matchMedia = () => activityViewport;
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
      if (name === 'recommendations' || name === 'search') return { items: [{ work_id: '/works/OL82563W', title: 'Harry Potter and the Philosopher’s Stone', authors: ['J. K. Rowling'], year: 1997, genres: ['fantasy', 'science_fiction'] }] };
      if (name === 'work_details') return { description: 'A long Harry Potter synopsis. '.repeat(300), genres: ['fantasy', 'science_fiction', 'fantasy', 'unknown_topic'] };
      if (name === 'editions') return { total: 30, items: Array.from({ length: 30 }, (_, i) => ({ edition_id: `/books/OL${i}M`, title: `Harry Potter edition ${i}`, language: 'eng', page_count: 300 })) };
      if (name === 'create_book') {
        const book = { ...args.b, id: books.length + 1, status: 'want', created_at: '2026-10-01', updated_at: '2026-10-01' };
        books.push(book); return book;
      }
      if (name === 'get_book') return { book: books.find(b => b.id === args.id), readings: sessions.has(args.id) ? [sessions.get(args.id)] : [] };
      if (name === 'start_reading') {
        const reading = { id: args.id, book_id: args.id, unit: args.input.unit, status: 'reading', current_value: 0 };
        sessions.set(args.id, { reading, progress: [] });
        books.find(b => b.id === args.id).status = 'reading'; return reading;
      }
      if (name === 'update_rating') {
        const book = books.find(b => b.id === args.id);
        book.rating = args.input.rating;
        return book;
      }
      if (name === 'change_reading') {
        const session = [...sessions.values()].find(s => s.reading.id === args.id);
        const book = books.find(b => b.id === session.reading.book_id);
        session.reading.status = book.status = args.input.status;
        if (args.input.status === 'completed') session.reading.current_value = session.reading.unit === 'percent' ? 100 : book.page_count;
        return session.reading;
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
    await waitFor(() => $('#activity-grid').children.length > 0);
    assert.equal($('.summary-section').tagName, 'DETAILS');
    assert.equal($('.summary-section').open, false);
    assert.equal($('.activity-scroll').style.getPropertyValue('--activity-weeks'), '13');
    assert.match($('#activity-summary').textContent, /last 13 weeks/);
    assert.match($('#activity-grid').getAttribute('aria-label'), /last 13 weeks/);
    activityViewport.matches = false;
    activityViewport.dispatchEvent(new window.Event('change'));
    assert.ok($('#activity-grid').children.length >= 365);
    assert.match($('#activity-summary').textContent, /last 12 months/);
    activityViewport.matches = true;
    activityViewport.dispatchEvent(new window.Event('change'));
    assert.equal($('.activity-scroll').style.getPropertyValue('--activity-weeks'), '13');
    $('#manual-add').click();
    $('#book-form').elements.title.value = 'Android UI test';
    $('#book-form').elements.page_count.value = '200';
    submit('#book-form');
    await waitFor(() => $('#detail-dialog').open);
    assert.ok($('[data-action="start"]'));
    $('#detail-dialog').close();
    const quickStart = $('[data-start-book="1"]');
    assert.ok(quickStart);
    quickStart.click();
    quickStart.click();
    await waitFor(() => $('#page-1'));
    assert.equal($('#detail-dialog').open, false);
    assert.equal(calls.filter(c => c.name === 'start_reading').length, 1);
    assert.equal(calls.find(c => c.name === 'start_reading').args.input.unit, 'pages');
    assert.equal($('[data-start-book="1"]'), null);
    $('.reading-book[data-open="1"]').click();
    await waitFor(() => $('#detail-dialog').open && $('[data-action="progress"]'));
    $('[data-action="progress"]').click();
    $('#progress-form').elements.value.value = '42';
    submit('#progress-form');
    await waitFor(() => $('#detail-dialog').open && $('#detail-content').textContent.includes('42 / 200 pages'));
    assert.equal($('#page-1').value, '42');
    assert.equal($('#page-1').nextElementSibling.textContent, '/');
    assert.equal($('#page-1').nextElementSibling.nextElementSibling.textContent, '200');
    assert.equal($('#page-1').parentElement.children.length, 3);

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
    await waitFor(() => $('[data-home-recommend]'));
    assert.equal($('.home-topics').textContent, 'Fantasy, Sci-fi');
    assert.equal($('.home-topics strong'), null);
    $('[data-home-recommend]').click();
    await waitFor(() => $('#browse-dialog-synopsis')?.textContent.startsWith('A long Harry Potter'));
    assert.deepEqual([...window.document.querySelectorAll('.browse-topic')].map(topic => topic.textContent), ['Fantasy', 'Sci-fi']);
    // Long descriptions and edition lists scroll independently of the footer.
    assert.equal($('#browse-dialog-actions').parentElement, $('#browse-dialog'));
    assert.equal($('#browse-dialog-content').closest('.dialog-scroll'), null);
    assert.equal($('#browse-view-editions').closest('.dialog-scroll'), null);
    $('#browse-view-editions').click();
    await waitFor(() => $('#browse-dialog-edition-results').children.length === 30);
    assert.equal($('#browse-dialog-editions').hidden, false);
    assert.equal($('#browse-dialog-content').hidden, true);
    assert.equal($('#browse-view-editions').textContent, 'Book details');
    assert.equal($('#browse-dialog-actions form').closest('.dialog-scroll'), null);
    $('#browse-view-editions').click();
    assert.equal($('#browse-dialog-content').hidden, false);
    assert.equal($('#browse-dialog-editions').hidden, true);
    assert.equal($('#browse-view-editions').textContent, 'View editions');
    assert.equal($('#browse-topic-list').textContent, 'FantasySci-fi');
    submit('#browse-dialog-actions form');
    await waitFor(() => calls.some(c => c.name === 'plugin:opener|open_url' && c.args.url.startsWith('https://www.amazon.es/s?')));
    $('#browse-dialog').close();
    $('[data-nav="browse"]').click();
    await waitFor(() => $('[data-recommend-work]'));
    assert.equal($('.recommend-topics').textContent, 'Fantasy, Sci-fi');
    assert.equal($('.recommend-topics strong'), null);
    $('[data-recommend-genre="fantasy"]').click();
    $('#recommend-load').click();
    await waitFor(() => $('.recommend-topics strong'));
    assert.equal($('.recommend-topics strong').textContent, 'Fantasy');
    assert.equal($('.recommend-topics').textContent, 'Fantasy, Sci-fi');
    $('[data-recommend-work]').click();
    await waitFor(() => $('#browse-topic-list strong'));
    assert.equal($('#browse-topic-list strong').textContent, 'Fantasy');
    $('#browse-dialog').close();
    $('#browse-search').value = 'Harry';
    $('#browse-search').dispatchEvent(new window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await waitFor(() => calls.some(c => c.name === 'search') && $('.recommend-topics'));
    assert.equal($('.recommend-topics strong'), null);
    $('[data-nav="inicio"]').click();
    await waitFor(() => $('.home-topics strong'));
    assert.equal($('.home-topics strong').textContent, 'Fantasy');
    assert.equal($('.home-topics').textContent, 'Fantasy, Sci-fi');
    $('[data-home-recommend]').click();
    await waitFor(() => $('#browse-topic-list strong'));
    assert.equal($('#browse-topic-list strong').textContent, 'Fantasy');
    $('#browse-dialog').close();
    $('#manual-add').click();
    $('#book-form').elements.title.value = 'Book without page count';
    $('#book-form').elements.page_count.value = '';
    submit('#book-form');
    await waitFor(() => $('#detail-dialog').open);
    assert.ok($('[data-action="start"]'));
    $('#detail-dialog').close();
    $('[data-start-book="2"]').click();
    await waitFor(() => $('#page-2'));
    assert.equal(calls.filter(c => c.name === 'start_reading').at(-1).args.input.unit, 'percent');
    assert.equal($('#page-2').nextElementSibling.textContent, '%');
    assert.equal($('#detail-dialog').open, false);
    $('.reading-book[data-open="1"]').click();
    await waitFor(() => $('#detail-dialog').open);
    $('#detail-rating').value = '8';
    $('#detail-rating').dispatchEvent(new window.Event('change', { bubbles: true }));
    await waitFor(() => calls.some(c => c.name === 'update_rating') && $('#detail-rating').value === '8');
    $('[data-status="completed"]').click();
    await waitFor(() => $('#home-books .book-row[data-open="1"]'));
    const finishedRow = $('#home-books .book-row[data-open="1"]');
    assert.equal(finishedRow.querySelector('.page-value').textContent, '200 / 200');
    assert.equal(finishedRow.querySelector('.row-rating').textContent, '★ 8/10');
    assert.equal(finishedRow.querySelector('.progress-caption'), null);
    assert.equal(finishedRow.querySelector('.progress-line').getAttribute('aria-valuenow'), '100');
    $('#detail-dialog').close();
    $('.reading-book[data-open="2"]').click();
    await waitFor(() => $('#detail-dialog').open);
    $('[data-status="completed"]').click();
    await waitFor(() => $('#home-books .book-row[data-open="2"]'));
    assert.equal($('#home-books .book-row[data-open="2"] .page-value').textContent, '100%');
    assert.equal($('#home-books .book-row[data-open="2"] .row-rating'), null);
    assert.equal(calls.some(c => c.name.includes('/api/')), false);
  } finally { window.close(); }
});
