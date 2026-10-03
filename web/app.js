import { library, catalog } from './services.js';
import { state } from './state.js';
import { topicGroups, popularTopics, topicSearchAliases, languages, topicLabel } from './topics.js';
import { confirmAction, saveBackup, loadBackup, installExternalLinks, installBackButton } from './platform.js';
import { validateBackup } from './backup.js';

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

const labels = { want: 'Want to read', reading: 'Reading', paused: 'Paused', completed: 'Finished', abandoned: 'Abandoned' };
let toastTimer, browseSearchTimer, isbnSerial = 0, recommendationSerial = 0, homeRecommendationSerial = 0, browseDialogSerial = 0, browseDialogEditionSerial = 0;
let backgroundScrollY = null;


function escapeHtml(value) { return String(value ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c])); }
function safeImage(value) { try { const u = new URL(value); return ['http:', 'https:'].includes(u.protocol) ? escapeHtml(u.href) : ''; } catch { return ''; } }
function languageLabel(value) {
  if (!value) return '';
  const names = {eng:'English',en:'English',por:'Portuguese',pt:'Portuguese','português':'Portuguese',spa:'Spanish',es:'Spanish','español':'Spanish',fre:'French',fra:'French',fr:'French','français':'French',ger:'German',deu:'German',de:'German','deutsch':'German',ita:'Italian',it:'Italian','italiano':'Italian'};
  return names[String(value).trim().toLocaleLowerCase('en')] || value;
}
function dateLabel(value) { if (!value) return ''; const d = new Date(String(value).replace(' ', 'T')); return Number.isNaN(d.getTime()) ? value : new Intl.DateTimeFormat('en-GB', {day:'numeric',month:'short',year:'numeric'}).format(d); }
function today() { return new Date().toLocaleDateString('sv-SE'); }
function notify(message, error = false) { const el = $('#toast'); el.textContent = message; el.className = `toast show${error ? ' error' : ''}`; clearTimeout(toastTimer); toastTimer = setTimeout(() => el.className = 'toast', 3500); }
function coverHtml(book, cls = '') { const src = safeImage(book.cover_url); return `<div class="${cls || 'cover-wrap'}">${src ? `<img src="${src}" alt="Cover of ${escapeHtml(book.title)}" loading="lazy"><span class="cover-fallback" hidden>${escapeHtml(book.title)}</span>` : `<span class="cover-fallback">${escapeHtml(book.title)}</span>`}</div>`; }
function progressPct(book, reading) { if (!reading) return 0; const max = reading.unit === 'percent' ? 100 : book.page_count; return max ? Math.min(100, Math.round(100 * reading.current_value / max)) : 0; }
function readingText(book, reading) { return reading.unit === 'percent' ? `${reading.current_value}%` : `${reading.current_value} / ${book.page_count || '?'} pages`; }
function empty(title, text, action = '') { return `<div class="empty-state"><strong>${escapeHtml(title)}</strong><p>${escapeHtml(text)}</p>${action}</div>`; }

async function refresh() {
  const [books, stats] = await Promise.all([library.list(), library.stats()]);
  const profile = items => items.map(({book}) => [book.id, book.work_id, book.rating, book.topics, book.status]);
  const oldProfile = JSON.stringify(profile(state.books));
  state.books = books; state.stats = stats;
  state.booksReady = true;
  if (oldProfile !== JSON.stringify(profile(books))) state.homeStale = true;
  renderDashboard(); renderLibrary(); renderRecommendations(); renderHomeRecommendations();
  if (state.view === 'inicio' && state.homeStale) loadHomeRecommendations();
}

function initRecommendations() {
  try {
    const saved = JSON.parse(localStorage.getItem('booklib-recommendation-filters') || '{}');
    state.recommendationGenres = (saved.genres || []).filter(value => popularTopics[value]).slice(0, 4);
    state.recommendationLanguages = (saved.languages || []).filter(value => languages[value]);
    const history = JSON.parse(localStorage.getItem('booklib-viewed-works') || '[]');
    state.viewedWorks = Array.isArray(history) ? history.filter(item => /^OL\d{1,12}W$/.test(item.id)).slice(0, 30) : [];
  } catch { /* Invalid or unavailable local storage: use empty filters. */ }
  renderRecommendationChoices();
}
function recordViewedWork(id, knownGenre = '') {
  if (!/^OL\d{1,12}W$/.test(id || '')) return;
  const existing = state.viewedWorks.find(item => item.id === id);
  const known = popularTopics[knownGenre] ? [knownGenre] : [];
  const entry = {id, genres:[...new Set([...(existing?.genres || []), ...known])], views:Math.min(5, (existing?.views || 0) + 1)};
  state.viewedWorks = [entry, ...state.viewedWorks.filter(item => item.id !== id)].slice(0, 30);
  state.homeStale = true;
  try { localStorage.setItem('booklib-viewed-works', JSON.stringify(state.viewedWorks)); } catch { /* History still works in this session. */ }
  if (entry.genres.length) return;
  catalog.genres(id).then(data => {
    const current = state.viewedWorks.find(item => item.id === id);
    if (!current) return;
    current.genres = [...new Set([...current.genres, ...(data.genres || []).filter(value => popularTopics[value])])];
    try { localStorage.setItem('booklib-viewed-works', JSON.stringify(state.viewedWorks)); } catch { /* Keep in memory. */ }
    state.homeStale = true;
  }).catch(() => {});
}
function renderRecommendationChoices() {
  $('#recommend-genres').innerHTML = renderTopicGroups('data-recommend-genre', state.recommendationGenres);
  filterTopicChoices($('#recommend-topic-search'), $('#recommend-genres'));
  $('#recommend-languages').innerHTML = Object.entries(languages).map(([key, label]) => `<button type="button" data-recommend-language="${key}" aria-pressed="${state.recommendationLanguages.includes(key)}">${label}</button>`).join('');
}
function saveRecommendationChoices() {
  recommendationSerial++;
  homeRecommendationSerial++;
  state.homeStale = true;
  try { localStorage.setItem('booklib-recommendation-filters', JSON.stringify({genres:state.recommendationGenres,languages:state.recommendationLanguages})); } catch { /* Filters still work in this session. */ }
  state.recommendations = [];
  renderRecommendationChoices();
  $('#recommend-books').innerHTML = '';
  $('#recommend-message').textContent = 'Filters changed. Update results to refresh the list.';
}
function browseTopicMarkup(topics, filters = state.view === 'browse' && $('#browse-search').value.trim() ? [] : state.recommendationGenres) {
  return [...new Set(topics.filter(topic => popularTopics[topic]))]
    .sort((a, b) => Number(filters.includes(b)) - Number(filters.includes(a)))
    .map(topic => filters.includes(topic) ? `<strong>${escapeHtml(topicLabel(topic))}</strong>` : `<span>${escapeHtml(topicLabel(topic))}</span>`);
}
function catalogTopics(item) {
  return [...new Set([...(item.genres || []), item.genre].filter(topic => popularTopics[topic]))];
}
function renderRecommendations() {
  const ownedIds = new Set(state.books.map(item => item.book.work_id).filter(Boolean));
  const ownedTitles = new Set(state.books.map(item => item.book.title.trim().toLocaleLowerCase()));
  const items = state.recommendations.filter(item => !ownedIds.has(item.work_id) && !ownedTitles.has(item.title.trim().toLocaleLowerCase()));
  $('#recommend-books').innerHTML = items.map((item, index) => `<button class="recommend-card" type="button" data-recommend-work="${index}" aria-label="Open ${escapeHtml(item.title)}">${coverHtml(item, 'mini-cover')}<span class="recommend-info"><strong>${escapeHtml(item.title)}</strong><span>${escapeHtml((item.authors || []).join(', ') || 'Unknown author')}</span><small>${escapeHtml(item.year || 'Book')}${item.ratings_count >= 5 ? `, ★ ${Number(item.ratings_average).toFixed(1)} (${item.ratings_count})` : ''}</small><span class="recommend-topics" aria-label="Book topics">${browseTopicMarkup(catalogTopics(item)).join(', ') || 'No topics listed'}</span></span></button>`).join('');
  state.visibleRecommendations = items;
}
function syncBrowseFilters() {
  const searching = Boolean($('#browse-search').value.trim());
  const toggle = $('#filter-toggle');
  toggle.hidden = searching;
  $('#recommend-controls').hidden = searching || toggle.getAttribute('aria-expanded') !== 'true';
}
async function loadRecommendations() {
  const term = $('#browse-search').value.trim();
  if (/^(?:\d{13}|\d{9}[\dXx])$/.test(term.replace(/[\s-]/g, ''))) {
    $('#isbn-query').value = term;
    return lookupIsbn(term);
  }
  showBrowseResults();
  const serial = ++recommendationSerial;
  syncBrowseFilters();
  if (term.length === 1) {
    state.recommendations = [];
    renderRecommendations();
    $('#recommend-message').textContent = 'Enter at least two characters to search.';
    return;
  }
  const button = $('#recommend-load'); button.disabled = true;
  $('#recommend-message').textContent = term ? 'Searching books…' : 'Finding books…';
  $('#recommend-books').innerHTML = '';
  try {
    const params = new URLSearchParams({genres:state.recommendationGenres.join(','),languages:state.recommendationLanguages.join(',')});
    const data = await (term ? catalog.search(term) : catalog.recommendations(Object.fromEntries(params)));
    if (serial !== recommendationSerial) return;
    state.recommendations = data.items;
    renderRecommendations();
    state.browseLoaded = true;
    $('#recommend-message').textContent = state.visibleRecommendations.length ? '' : term ? 'No books found. Try another title or author.' : 'No books match these filters. Try another genre or fewer languages.';
  } catch (error) { if (serial === recommendationSerial) { $('#recommend-message').textContent = error.message; notify(error.message, true); } }
  finally { button.disabled = false; }
}
function renderHomeRecommendations() {
  const owned = new Set(state.books.map(item => item.book.work_id).filter(Boolean));
  const items = state.homeRecommendations.filter(item => !owned.has(item.work_id)).slice(0, 10);
  state.visibleHomeRecommendations = items;
  $('#home-recommend-books').innerHTML = items.map((item, index) => `<article class="home-recommend-card"><button class="home-recommend-main" type="button" data-home-recommend="${index}" aria-label="Open details for ${escapeHtml(item.title)}">${coverHtml(item, 'home-cover')}<strong>${escapeHtml(item.title)}</strong><span>${escapeHtml((item.authors || []).join(', ') || 'Unknown author')}</span><small>${item.ratings_count >= 5 ? `★ ${Number(item.ratings_average).toFixed(1)}` : 'Popular book'}</small><span class="home-topics" aria-label="Book topics">${browseTopicMarkup(catalogTopics(item), state.recommendationGenres).join(', ') || 'No topics listed'}</span></button><form action="https://www.amazon.es/s" method="get" target="_blank" rel="noopener noreferrer"><input type="hidden" name="k" value="${escapeHtml(item.title)}"><input type="hidden" name="i" value="stripbooks"><button class="home-recommend-price" type="submit" aria-label="Search prices for ${escapeHtml(item.title)} on Amazon">Prices</button></form></article>`).join('');
}
async function loadHomeRecommendations() {
  if (!state.booksReady) return;
  const serial = ++homeRecommendationSerial;
  state.homeStale = false;
  $('#home-recommend-message').textContent = 'Finding books…';
  const workIds = [...new Set([...state.viewedWorks.map(item => item.id), ...state.books.map(item => item.book.work_id).filter(Boolean)])].slice(0, 30);
  const interests = {};
  for (const item of state.viewedWorks) for (const genre of item.genres || []) if (popularTopics[genre]) interests[genre] = (interests[genre] || 0) + Math.min(5, item.views || 1);
  for (const {book} of state.books) {
    const weight = book.rating == null ? 2 : book.rating - 5;
    for (const topic of (book.topics || '').split(',')) if (weight && popularTopics[topic]) interests[topic] = (interests[topic] || 0) + weight;
  }
  const params = new URLSearchParams({mode:'home',seen:workIds.join(','),interests:Object.entries(interests).filter(([,count]) => count > 0).map(([genre,count]) => `${genre}:${count}`).join(','),avoid:Object.entries(interests).filter(([,count]) => count < 0).map(([genre]) => genre).join(','),genres:state.recommendationGenres.join(','),languages:state.recommendationLanguages.join(',')});
  try {
    const data = await catalog.recommendations(Object.fromEntries(params));
    if (serial !== homeRecommendationSerial) return;
    state.homeRecommendations = data.items;
    renderHomeRecommendations();
    $('#home-recommend-message').textContent = state.visibleHomeRecommendations.length ? '' : 'No matches yet. Explore more in Browse.';
  } catch { if (serial === homeRecommendationSerial) $('#home-recommend-message').textContent = 'Suggestions unavailable right now. Try Browse.'; }
}
const activityMobile = window.matchMedia?.('(max-width: 700px)');
activityMobile?.addEventListener('change', () => { if (state.stats) renderReadingActivity(); });

function renderReadingActivity() {
  const end = new Date();
  end.setHours(0, 0, 0, 0);
  const first = new Date(end);
  const compact = activityMobile?.matches;
  const period = compact ? 'the last 13 weeks' : 'the last 12 months';
  first.setDate(first.getDate() - (compact ? 84 + end.getDay() : 364));
  const start = new Date(first);
  start.setDate(start.getDate() - start.getDay());
  const dateKey = date => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
  const firstKey = dateKey(first);
  const totals = new Map((state.stats.daily_pages || []).map(item => [item.date, Number(item.pages) || 0]));
  const days = [], months = [];
  let pagesRead = 0, activeDays = 0;
  for (const day = new Date(start); day <= end; day.setDate(day.getDate() + 1)) {
    const date = dateKey(day), pages = totals.get(date) || 0;
    const level = pages >= 50 ? 4 : pages >= 25 ? 3 : pages >= 10 ? 2 : pages > 0 ? 1 : 0;
    const label = `${date}, ${pages} ${pages === 1 ? 'page' : 'pages'}`;
    days.push(`<span class="activity-day activity-level-${level}" title="${label}" ${pages ? `role="img" aria-label="${label}"` : 'aria-hidden="true"'}></span>`);
    if (day.getDate() === 1) months.push(`<span style="grid-column:${Math.floor((days.length - 1) / 7) + 1}">${day.toLocaleString('en', {month:'short'})}</span>`);
    if (date >= firstKey) { pagesRead += pages; if (pages) activeDays++; }
  }
  $('#activity-grid').innerHTML = days.join('');
  $('.activity-scroll').style.setProperty('--activity-weeks', Math.ceil(days.length / 7));
  $('#activity-grid').setAttribute('aria-label', `Pages read by day in ${period}`);
  $('#activity-months').innerHTML = months.join('');
  $('#activity-summary').textContent = `${pagesRead} pages, ${activeDays} active ${activeDays === 1 ? 'day' : 'days'} in ${period}`;
}
function renderDashboard() {
  const st = state.stats;
  $('#stats').innerHTML = [
    ['Books in library', st.total || 0], ['Currently reading', st.reading || 0],
    ['Finished', st.completed || 0], ['Pages read', st.pages_read || 0]
  ].map(([label, value]) => `<div class="stat-card"><span class="stat-label">${label}</span><strong class="stat-number">${value}</strong></div>`).join('');
  renderReadingActivity();
  const active = state.books
    .filter(({book, reading}) => reading && ['reading', 'paused'].includes(book.status))
    .sort((a, b) => b.reading.id - a.reading.id);
  const planned = state.books
    .filter(({book}) => book.status === 'want')
    .sort((a, b) => b.book.id - a.book.id);
  const shelf = state.books
    .filter(({book}) => !['reading', 'paused', 'want'].includes(book.status))
    .sort((a, b) => b.book.id - a.book.id);
  $('.shelf-section').hidden = shelf.length === 0;
  $('.summary-section').hidden = state.books.length === 0;
  $('#currently-reading').innerHTML = active.length ? active.map(readingCard).join('') : empty('No books in progress', 'Add a book or start reading one already in your library.', '<a class="secondary-button" href="#browse">Browse books</a>');
  $('#planned-books').innerHTML = planned.length ? planned.map(plannedCard).join('') : empty('No books planned yet', 'Find a book to read next.', '<a class="secondary-button" href="#browse">Browse books</a>');
  $('#home-books').innerHTML = shelf.map(bookRow).join('');
}
function plannedCard({book}) {
  return `<article class="planned-card" data-open="${book.id}"><button class="planned-main" type="button" aria-label="Open details for ${escapeHtml(book.title)}">${coverHtml(book, 'planned-cover')}<span class="planned-info"><strong>${escapeHtml(book.title)}</strong><span>${escapeHtml(book.authors || 'Unknown author')}</span></span></button><div class="planned-actions"><button class="planned-start" type="button" data-start-book="${book.id}">Start reading</button><form class="planned-buy-form" action="https://www.amazon.es/s" method="get" target="_blank" rel="noopener noreferrer"><input type="hidden" name="k" value="${escapeHtml(book.title)}"><input type="hidden" name="i" value="stripbooks"><button class="planned-buy" type="submit" aria-label="Search prices for ${escapeHtml(book.title)} on Amazon">Prices</button></form></div></article>`;
}
function readingCard({book, reading}) {
  const max = reading.unit === 'percent' ? 100 : book.page_count;
  const unit = reading.unit === 'percent' ? 'percentage point' : 'page';
  const digits = Math.max(String(max || 0).length, String(reading.current_value).length, 2);
  const numberUnits = reading.unit === 'percent' ? digits * 0.6 + 1.2 : digits * 1.2 + 0.6;
  return `<article class="reading-card"><button class="reading-book" type="button" data-open="${book.id}" aria-label="Open details for ${escapeHtml(book.title)}">${coverHtml(book, 'reading-cover')}<strong>${escapeHtml(book.title)}</strong><span class="reading-author">${escapeHtml(book.authors || 'Unknown author')}</span></button><div class="reading-page"><label for="page-${book.id}">${reading.unit === 'percent' ? 'Progress' : 'Page'}</label><div class="page-controls"><div class="page-number${reading.unit === 'percent' ? ' page-number-percent' : ''}" style="--page-number-units:${numberUnits}"><input id="page-${book.id}" data-page-input="${book.id}" type="number" inputmode="numeric" min="0" max="${max}" value="${reading.current_value}" aria-label="Current ${reading.unit === 'percent' ? 'percentage' : 'page'} for ${escapeHtml(book.title)}">${reading.unit === 'percent' ? '<span class="page-total">%</span>' : `<span class="page-separator" aria-hidden="true">/</span><span class="page-total" aria-label="Total pages">${max || '—'}</span>`}</div><div class="step-buttons"><button type="button" data-step="1" data-book="${book.id}" aria-label="Increase ${unit} for ${escapeHtml(book.title)}" ${reading.current_value >= max ? 'disabled' : ''}>+</button><button type="button" data-step="-1" data-book="${book.id}" aria-label="Decrease ${unit} for ${escapeHtml(book.title)}" ${reading.current_value <= 0 ? 'disabled' : ''}>−</button></div></div><div class="progress-line" role="progressbar" aria-valuenow="${progressPct(book, reading)}" aria-valuemin="0" aria-valuemax="100"><span style="width:${progressPct(book, reading)}%"></span></div></div></article>`;
}
async function saveInlineProgress(bookId, value) {
  const item = state.books.find(({book}) => book.id === bookId);
  if (!item?.reading || state.pendingProgress.has(bookId)) return;
  const max = item.reading.unit === 'percent' ? 100 : item.book.page_count;
  if (!Number.isInteger(value) || value < 0 || value > max) { notify(`Enter a value between 0 and ${max}`, true); renderDashboard(); return; }
  if (value === item.reading.current_value) return;
  state.pendingProgress.add(bookId);
  $$('[data-book], [data-page-input]').filter(el => Number(el.dataset.book || el.dataset.pageInput) === bookId).forEach(el => el.disabled = true);
  try {
    await library.addProgress(item.reading.id, {value, recorded_at: today()});
    await refresh();
  } catch (error) { notify(error.message, true); renderDashboard(); }
  finally { state.pendingProgress.delete(bookId); }
}
function bookRow({book, reading}) {
  const active = reading && ['reading', 'paused'].includes(book.status);
  const progress = reading ? `<div class="row-progress"><div class="row-progress-head"><strong class="page-value">${reading.unit === 'percent' ? `${reading.current_value}<small>%</small>` : `${reading.current_value} / ${book.page_count || '?'}`}</strong></div><div class="progress-line" role="progressbar" aria-valuenow="${progressPct(book, reading)}" aria-valuemin="0" aria-valuemax="100"><span style="width:${progressPct(book, reading)}%"></span></div></div>` : `<div class="row-progress row-progress-empty"><strong>Not started</strong></div>`;
  return `<article class="book-row" data-open="${book.id}" tabindex="0" aria-label="Open ${escapeHtml(book.title)}">${coverHtml(book, 'row-cover')}<div class="row-main"><h3><button class="row-title-button" type="button">${escapeHtml(book.title)}</button></h3><p>${escapeHtml(book.authors || 'Unknown author')}</p><span class="pill ${book.status}">${labels[book.status] || 'Book'}</span>${book.rating != null ? `<span class="row-rating" aria-label="Your rating: ${book.rating} out of 10">★ ${book.rating}/10</span>` : ''}</div>${progress}<div class="row-action">${active ? `<button class="row-update" type="button" data-progress="${book.id}" aria-label="Update progress for ${escapeHtml(book.title)}">Update <span aria-hidden="true">↗</span></button>` : '<span aria-hidden="true">→</span>'}</div></article>`;
}
function renderLibrary() {
  const term = $('#library-search').value.trim().toLocaleLowerCase('en');
  let rows = state.books.filter(({book}) => (state.filter === 'all' || book.status === state.filter) && `${book.title} ${book.authors} ${book.tags}`.toLocaleLowerCase('en').includes(term));
  const sort = $('#library-sort').value;
  if (sort === 'recent') rows = rows.sort((a, b) => b.book.id - a.book.id);
  if (sort === 'title') rows = rows.sort((a, b) => a.book.title.localeCompare(b.book.title, 'en'));
  if (sort === 'author') rows = rows.sort((a, b) => a.book.authors.localeCompare(b.book.authors, 'en'));
  $('#library-count').textContent = `${rows.length} ${rows.length === 1 ? 'book' : 'books'}`;
  $('#library-books').innerHTML = rows.length ? rows.map(bookRow).join('') : empty('No books here', term || state.filter !== 'all' ? 'Try another search or filter.' : 'Add a book from the catalog or create one manually.', '<a class="secondary-button" href="#browse">Find books</a>');
}
function navigate() {
  const previousView = state.view;
  let view = location.hash.slice(1) || 'inicio';
  if (view === 'descobrir') { history.replaceState(null, '', '#browse'); view = 'browse'; }
  state.view = ['inicio','biblioteca','browse'].includes(view) ? view : 'inicio';
  $$('.view').forEach(el => el.hidden = el.id !== `view-${state.view}`);
  $$('[data-nav]').forEach(el => el.classList.toggle('active', el.dataset.nav === state.view || (state.view === 'biblioteca' && el.dataset.nav === 'inicio')));
  if (state.view === 'browse' && previousView !== 'browse') showBrowseResults();
  window.scrollTo({top:0,behavior:'instant'});
  if (state.view === 'browse' && !state.browseLoaded) loadRecommendations();
  if (state.view === 'inicio' && state.booksReady && state.homeStale) loadHomeRecommendations();
}
function openDialog(dialog) {
  if (dialog.open) return;
  if (backgroundScrollY === null) {
    backgroundScrollY = window.scrollY;
    document.body.style.setProperty('--locked-scroll-top', `-${backgroundScrollY}px`);
    document.body.style.setProperty('--scrollbar-gap', `${Math.max(0, window.innerWidth - document.documentElement.clientWidth)}px`);
    document.body.classList.add('dialog-open');
  }
  dialog.showModal();
  $('.dialog-scroll', dialog)?.scrollTo(0, 0);
}
function restoreBackgroundScroll() {
  if (backgroundScrollY === null || $('dialog[open]')) return;
  const scrollY = backgroundScrollY;
  backgroundScrollY = null;
  document.body.classList.remove('dialog-open');
  document.body.style.removeProperty('--locked-scroll-top');
  document.body.style.removeProperty('--scrollbar-gap');
  window.scrollTo({top: scrollY, behavior: 'instant'});
}
function closeDialog(dialog) { dialog.close(); }
function bookPayload(form) {
  const data = Object.fromEntries(new FormData(form));
  return {title:data.title.trim(), authors:data.authors.trim(), isbn:data.isbn || null,
    work_id:state.bookEdit?.work_id || null, edition_id:state.bookEdit?.edition_id || null,
    cover_url:data.cover_url || null, page_count:data.page_count ? Number(data.page_count) : null,
    language:data.language || null, published:data.published || null, description:data.description || '',
    source:state.bookEdit?.source || 'manual', rating:state.bookEdit?.rating ?? null,
    review:data.review || '', notes:data.notes || '', tags:data.tags || '',
    topics:$$('[data-book-topic][aria-pressed="true"]', form).map(button => button.dataset.bookTopic).join(',')};
}
function renderBookTopics(selected = []) {
  $('#book-topic-choices').innerHTML = renderTopicGroups('data-book-topic', selected);
  filterTopicChoices($('#book-topic-search'), $('#book-topic-choices'));
  $('#book-topic-summary').textContent = selected.length ? `Book topics (${selected.length} selected)` : 'Book topics (none selected)';
}
function renderTopicGroups(attribute, selected) {
  return topicGroups.map(group => `<section class="topic-group"><h4>${group.label}</h4><div class="choice-row">${Object.entries(group.topics).map(([key, label]) => `<button type="button" ${attribute}="${key}" aria-pressed="${selected.includes(key)}">${label}</button>`).join('')}</div></section>`).join('');
}
function filterTopicChoices(input, root) {
  const normalise = value => value.toLocaleLowerCase().normalize('NFD').replace(/[\u0300-\u036f]/g, '');
  const query = normalise(input.value.trim());
  $$('.topic-group', root).forEach(group => {
    const buttons = $$('button', group);
    buttons.forEach(button => { button.hidden = Boolean(query) && !normalise(`${button.textContent} ${topicSearchAliases[button.dataset.bookTopic || button.dataset.recommendGenre] || ''}`).includes(query); });
    group.hidden = buttons.every(button => button.hidden);
  });
}
function openBookForm(book = null) {
  state.bookEdit = book;
  $('#book-form').reset();
  $('#book-topic-search').value = '';
  $('#book-form-kicker').textContent = book ? 'Edit entry' : 'New book';
  $('#book-form-heading').textContent = book ? 'Edit book' : 'Add to library';
  $('#book-form button[type="submit"]').textContent = 'Save book';
  if (book) for (const name of ['title','authors','isbn','page_count','language','published','cover_url','description','review','notes','tags']) $('#book-form').elements[name].value = book[name] ?? '';
  renderBookTopics((book?.topics || '').split(',').filter(value => popularTopics[value]));
  $('.book-topic-picker').open = Boolean(book && !book.topics);
  openDialog($('#book-dialog'));
}
async function openDetail(id) {
  state.selected = await library.get(id); recordViewedWork(state.selected.book.work_id); renderDetail(); openDialog($('#detail-dialog'));
}
function renderDetail() {
  const {book, readings} = state.selected;
  const latest = readings[0]?.reading;
  const active = latest && ['reading','paused'].includes(latest.status);
  const progressControls = active
    ? `<button class="primary-button" data-action="progress">Update progress</button>${latest.status === 'paused' ? '<button class="secondary-button" data-status="reading">Resume</button>' : '<button class="secondary-button" data-status="paused">Pause</button>'}<button class="secondary-button" data-status="completed">Finish</button><button class="secondary-button" data-status="abandoned">Abandon</button>`
    : `<select id="unit-select" aria-label="Progress unit">${book.page_count ? '<option value="pages">Pages</option>' : ''}<option value="percent">Percentage</option></select><button class="primary-button" data-action="start">${readings.length ? 'Read again' : 'Start reading'}</button>${latest ? '<button type="button" class="secondary-button" data-action="correct-progress">Edit progress</button>' : ''}`;
  const metadata = [languageLabel(book.language), book.published, book.page_count ? `${book.page_count} pages` : null, book.isbn ? `ISBN ${book.isbn}` : null].filter(Boolean).map(escapeHtml).join(', ');
  const pencil = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 20h4L19 9l-4-4L4 16v4Z"/><path d="m13 7 4 4"/></svg>';
  const bin = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M10 7V4h4v3m4 0-1 13H7L6 7M10 10v7m4-7v7"/></svg>';
  const ratingOptions = ['<option value="">No rating</option>', ...Array.from({length:10}, (_, i) => `<option value="${i + 1}"${book.rating === i + 1 ? ' selected' : ''}>${i + 1} / 10</option>`)].join('');
  $('#detail-content').innerHTML = `
    <div class="detail-top">${coverHtml(book, 'detail-cover')}<div class="detail-info">
      <div class="detail-title-row"><h2>${escapeHtml(book.title)}</h2><div class="detail-title-actions"><button class="detail-icon-button" type="button" data-action="edit-book" aria-label="Edit ${escapeHtml(book.title)}" title="Edit book">${pencil}</button><button class="detail-icon-button danger" type="button" data-action="delete-book" aria-label="Remove ${escapeHtml(book.title)}" title="Remove book">${bin}</button></div></div>
      <p>${escapeHtml(book.authors || 'Unknown author')}</p><p>${metadata}</p>
      ${book.topics ? `<p class="detail-topics">${escapeHtml(book.topics.split(',').map(topicLabel).join(', '))}</p>` : ''}
      <label class="detail-rating">Your rating <select id="detail-rating" aria-label="Your rating from 1 to 10">${ratingOptions}</select></label>
      ${latest ? `<div class="progress-line"><span style="width:${progressPct(book,latest)}%"></span></div><p>${readingText(book, latest)}</p>` : ''}
      <div class="detail-actions">${progressControls}</div>
    </div></div>
    ${book.description ? `<section class="detail-section"><h3>About this book</h3><p>${escapeHtml(book.description)}</p></section>` : ''}
    ${book.tags ? `<section class="detail-section"><h3>Tags</h3><p>${escapeHtml(book.tags)}</p></section>` : ''}
    ${book.review ? `<section class="detail-section"><h3>Your review</h3><p>${escapeHtml(book.review)}</p></section>` : ''}
    ${book.notes ? `<section class="detail-section"><h3>Notes</h3><p>${escapeHtml(book.notes)}</p></section>` : ''}
    ${readings.length ? `<details class="detail-section reading-history"><summary>Reading history</summary>${readings.map(({reading, progress}, index) => `<section class="reading-history-session"><div class="reading-history-head"><strong>${labels[reading.status]}</strong><span>${escapeHtml(reading.started_at)}${reading.finished_at ? ` → ${escapeHtml(reading.finished_at)}` : ''}</span></div>${progress.map(entry => `<div class="reading-history-entry"><span>${escapeHtml(entry.recorded_at.slice(0,10))}</span><span>${entry.value}${reading.unit === 'percent' ? '%' : ` / ${book.page_count || '?'}`}</span><button type="button" class="secondary-button" data-edit-progress="${entry.id}" aria-label="Edit progress on ${escapeHtml(entry.recorded_at.slice(0,10))}">Edit</button></div>`).join('')}${index === 0 ? `<label class="reading-status-edit">Status<select data-edit-reading-status="${reading.id}" aria-label="Reading status">${['reading','paused','completed','abandoned'].map(status => `<option value="${status}"${reading.status === status ? ' selected' : ''}>${labels[status]}</option>`).join('')}</select></label><button type="button" class="secondary-button danger" data-remove-reading="${reading.id}">Remove this reading</button>` : ''}</section>`).join('')}</details>` : ''}`;
}
function openProgress(entry = null) {
  const {book, readings} = state.selected; const r = readings.find(x => x.progress.some(p => p.id === entry?.id))?.reading || readings[0]?.reading;
  if (!r) return;
  state.progressEdit = entry;
  $('#progress-form').reset();
  $('#progress-heading').textContent = entry ? 'Edit progress' : 'Update progress';
  $('#progress-context').textContent = `${book.title}, ${r.unit === 'percent' ? 'percentage (0–100)' : `page (0–${book.page_count})`}`;
  $('#progress-form').elements.value.max = r.unit === 'percent' ? 100 : book.page_count;
  $('#progress-form').elements.value.value = entry?.value ?? r.current_value;
  $('#progress-form').elements.note.value = entry?.note || '';
  $('#progress-form').elements.recorded_at.value = entry?.recorded_at?.slice(0,10) || today();
  if ($('#detail-dialog').open) $('#detail-dialog').close();
  openDialog($('#progress-dialog'));
}

function showBrowseResults() {
  isbnSerial++;
  $('#edition-area').hidden = true;
  $('#browse-recommend-section').hidden = false;
  $('#isbn-message').hidden = true;
}
function renderBrowseTopics(topics, emptyMessage = 'No topics listed') {
  const known = [...new Set(topics.filter(topic => popularTopics[topic]))];
  $('#browse-topic-list').innerHTML = known.length
    ? browseTopicMarkup(known).map(topic => `<span class="browse-topic" role="listitem">${topic}</span>`).join('')
    : `<span class="muted">${escapeHtml(emptyMessage)}</span>`;
}
async function openBrowseWork(item) {
  if (!item) return;
  clearTimeout(browseSearchTimer);
  state.catalogItems = [item];
  state.work = item;
  state.editionOffset = 0;
  $('#browse-dialog-content').hidden = false;
  $('#browse-dialog-editions').hidden = true;
  $('#browse-dialog-edition-results').innerHTML = '';
  const serial = ++browseDialogSerial;
  browseDialogEditionSerial++;
  const knownTopics = [...new Set([...(state.viewedWorks.find(entry => entry.id === item.work_id)?.genres || []), ...catalogTopics(item)].filter(topic => popularTopics[topic]))];
  const meta = item.year || '';
  $('#browse-dialog-content').innerHTML = `<div class="browse-dialog-top">${coverHtml(item, 'browse-dialog-cover')}<div class="browse-dialog-info"><h2>${escapeHtml(item.title)}</h2><p>${escapeHtml((item.authors || []).join(', ') || 'Unknown author')}</p>${meta ? `<p>${escapeHtml(meta)}</p>` : ''}${item.ratings_count >= 5 ? `<p>★ ${Number(item.ratings_average).toFixed(1)}, ${item.ratings_count} ratings</p>` : ''}</div></div><section class="browse-dialog-topics" aria-label="Book topics"><h3>Topics</h3><div id="browse-topic-list" class="browse-topic-list" role="list"></div></section><section class="browse-dialog-description"><h3>Synopsis</h3><p id="browse-dialog-synopsis" class="browse-synopsis">Loading synopsis…</p></section>`;
  $('#browse-dialog-actions').innerHTML = `<button id="browse-view-editions" class="secondary-button" type="button">View editions</button><form action="https://www.amazon.es/s" method="get" target="_blank" rel="noopener noreferrer"><input type="hidden" name="k" value="${escapeHtml(item.title)}"><input type="hidden" name="i" value="stripbooks"><button class="secondary-button" type="submit" aria-label="Search prices for ${escapeHtml(item.title)} on Amazon">Prices</button></form>`;
  renderBrowseTopics(knownTopics, 'Loading topics…');
  recordViewedWork(item.work_id, item.genre);
  openDialog($('#browse-dialog'));
  try {
    const details = await catalog.details(item.work_id);
    if (serial === browseDialogSerial && $('#browse-dialog').open) {
      $('#browse-dialog-synopsis').textContent = details.description || 'No synopsis available for this book.';
      renderBrowseTopics([...(details.genres || []), ...knownTopics]);
    }
  } catch {
    if (serial === browseDialogSerial && $('#browse-dialog').open) {
      $('#browse-dialog-synopsis').textContent = 'Synopsis unavailable right now.';
      renderBrowseTopics(knownTopics, 'Topics unavailable right now.');
    }
  }
}
async function loadBrowseDialogEditions(append = false) {
  if (!state.work) return;
  const serial = ++browseDialogEditionSerial;
  const work = state.work;
  const area = $('#browse-dialog-editions');
  $('#browse-dialog-content').hidden = true;
  area.hidden = false;
  $('#browse-view-editions').textContent = 'Book details';
  if (!append) { state.editionOffset = 0; $('#browse-dialog-edition-results').innerHTML = '<p class="muted">Loading editions…</p>'; }
  $('#browse-dialog-more-editions').hidden = true;
  try {
    const data = await catalog.editions(work.work_id, state.editionOffset);
    if (serial !== browseDialogEditionSerial || !$('#browse-dialog').open) return;
    const sorted = [...data.items].sort((a, b) => {
      const score = ed => (state.recommendationLanguages.length
        ? (state.recommendationLanguages.includes(ed.language) ? 0 : 1)
        : (ed.language === 'eng' ? 0 : ed.language === 'por' ? 1 : 2)) * 10
        + (ed.page_count ? 0 : 2) + (ed.cover_url ? 0 : 1);
      return score(a) - score(b);
    });
    const base = append ? state.editionItems.length : 0;
    state.editionItems = append ? [...state.editionItems, ...sorted] : sorted;
    if (!append) $('#browse-dialog-edition-results').innerHTML = '';
    $('#browse-dialog-edition-results').insertAdjacentHTML('beforeend', sorted.map((ed, i) => editionHtml(ed, base + i, work.title)).join('') || (append ? '' : empty('No editions available', 'You can add this book manually.')));
    state.editionOffset += data.items.length;
    $('#browse-dialog-more-editions').hidden = state.editionOffset >= data.total || !data.items.length;
  } catch (error) {
    if (serial === browseDialogEditionSerial) $('#browse-dialog-edition-results').innerHTML = empty('Editions unavailable', 'Try again later.');
  }
}
function editionHtml(ed, index, fallbackTitle) {
  return `<article class="edition-item">${coverHtml({...ed,title:ed.title || fallbackTitle},'mini-cover')}<div class="catalog-meta"><h3>${escapeHtml(ed.title || fallbackTitle)}</h3>${ed.authors?.length ? `<p>${escapeHtml(ed.authors.join(', '))}</p>` : ''}<p>${escapeHtml([languageLabel(ed.language),ed.published,ed.page_count ? `${ed.page_count} pages` : 'Page count unknown'].filter(Boolean).join(', '))}</p><p>${ed.isbn ? `ISBN ${escapeHtml(ed.isbn)}` : 'No ISBN'}</p></div><button class="primary-button" data-edition="${index}" type="button">Add</button></article>`;
}
async function lookupIsbn(raw) {
  clearTimeout(browseSearchTimer);
  const isbn = raw.replace(/[\s-]/g, '').toUpperCase();
  const serial = ++isbnSerial;
  recommendationSerial++;
  $('#isbn-message').hidden = false;
  $('#isbn-message').textContent = 'Finding the exact edition…';
  $('#browse-recommend-section').hidden = true;
  $('#edition-area').hidden = false;
  $('#edition-heading').textContent = 'Finding edition…';
  $('#edition-results').innerHTML = '';
  $('#more-editions').hidden = true;
  try {
    const ed = await catalog.isbn(isbn);
    if (serial !== isbnSerial) return;
    state.work = {title:ed.title, authors:ed.authors || [], cover_url:ed.cover_url, work_id:ed.work_id};
    state.editionItems = [ed]; state.editionOffset = 0; state.editionTotal = 1;
    $('#edition-heading').textContent = ed.title || 'Edition';
    $('#edition-results').innerHTML = editionHtml(ed, 0, ed.title || 'Untitled');
    $('#isbn-message').textContent = `Exact edition found, ISBN ${ed.isbn}`;
    recordViewedWork(ed.work_id);
  } catch (error) {
    if (serial !== isbnSerial) return;
    $('#edition-area').hidden = true;
    $('#browse-recommend-section').hidden = false;
    $('#isbn-message').textContent = error.message;
  }
}
async function showEditions(index, append = false) {
  if (!append) { clearTimeout(browseSearchTimer); state.work = state.catalogItems[index]; state.editionOffset = 0; $('#edition-results').innerHTML = ''; $('#isbn-query').value = ''; recordViewedWork(state.work?.work_id, state.work?.genre); }
  if (!state.work) return;
  isbnSerial++;
  $('#isbn-message').hidden = true;
  $('#edition-area').hidden = false; $('#browse-recommend-section').hidden = true;
  $('#more-editions').hidden = true;
  $('#edition-heading').textContent = state.work.title;
  $('#edition-results').insertAdjacentHTML('beforeend', append ? '' : '<p class="muted">Loading editions…</p>');
  try {
    const data = await catalog.editions(state.work.work_id, state.editionOffset);
    if (!append) $('#edition-results').innerHTML = '';
    const sorted = [...data.items].sort((a,b) => {
      const score = ed => (state.recommendationLanguages.length
        ? (state.recommendationLanguages.includes(ed.language) ? 0 : 1)
        : (ed.language === 'eng' ? 0 : ed.language === 'por' ? 1 : 2)) * 10
        + (ed.page_count ? 0 : 2) + (ed.cover_url ? 0 : 1);
      return score(a) - score(b);
    });
    state.editionTotal = data.total; const base = append ? state.editionItems.length : 0; state.editionItems = append ? [...state.editionItems, ...sorted] : sorted;
    $('#edition-results').insertAdjacentHTML('beforeend', sorted.map((ed, i) => editionHtml(ed, base + i, state.work.title)).join('') || (append ? '' : empty('No editions available', 'You can add this book manually.')));
    state.editionOffset += data.items.length;
    $('#more-editions').hidden = state.editionOffset >= data.total || !data.items.length;
  } catch (e) { $('#edition-results').innerHTML = empty('Editions unavailable', 'Try again or add the book manually.'); notify(e.message, true); }
}
async function importEdition(index) {
  const ed = state.editionItems[index]; if (!ed) return;
  const payload = {title:ed.title || state.work.title, authors:(state.work.authors || []).join(', '), isbn:ed.isbn || null, work_id:ed.work_id, edition_id:ed.edition_id, cover_url:ed.cover_url || state.work.cover_url, page_count:ed.page_count || null, language:ed.language || null, published:ed.published || null, description:'', source:'openlibrary', rating:null, review:'', notes:'', tags:''};
  if (!payload.page_count) {
    state.pendingEdition = payload;
    $('#page-count-form').reset();
    $('#page-count-book').textContent = payload.title;
    openDialog($('#page-count-dialog'));
    $('#page-count-form').elements.page_count.focus();
    return;
  }
  await createImportedBook(payload);
}
async function createImportedBook(payload) {
  if (state.importing) return;
  state.importing = true;
  $$('#page-count-form button').forEach(button => button.disabled = true);
  try {
    const book = await library.create(payload);
    if ($('#page-count-dialog').open) $('#page-count-dialog').close();
    if ($('#browse-dialog').open) $('#browse-dialog').close();
    state.pendingEdition = null;
    await refresh();
    notify('Book added to your library');
    await openDetail(book.id);
  } catch (error) { notify(error.message, true); }
  finally { state.importing = false; $$('#page-count-form button').forEach(button => button.disabled = false); }
}

$$('dialog').forEach(dialog => {
  dialog.addEventListener('close', restoreBackgroundScroll);
  dialog.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const rect = dialog.getBoundingClientRect();
    if (event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom) dialog.close();
  });
});

document.addEventListener('click', async event => {
  const close = event.target.closest('.close-dialog'); if (close) { close.closest('dialog').close(); return; }
  const genre = event.target.closest('[data-recommend-genre]');
  if (genre) { const key = genre.dataset.recommendGenre; if (state.recommendationGenres.includes(key)) state.recommendationGenres = state.recommendationGenres.filter(value => value !== key); else if (state.recommendationGenres.length < 4) state.recommendationGenres.push(key); else { notify('Choose up to 4 topics', true); return; } saveRecommendationChoices(); return; }
  const language = event.target.closest('[data-recommend-language]');
  if (language) { const key = language.dataset.recommendLanguage; state.recommendationLanguages = state.recommendationLanguages.includes(key) ? state.recommendationLanguages.filter(value => value !== key) : [...state.recommendationLanguages, key]; saveRecommendationChoices(); return; }
  const bookTopic = event.target.closest('[data-book-topic]');
  if (bookTopic) { const selected = $$('[data-book-topic][aria-pressed="true"]', $('#book-form')).map(button => button.dataset.bookTopic); const key = bookTopic.dataset.bookTopic; if (selected.includes(key)) renderBookTopics(selected.filter(value => value !== key)); else if (selected.length < 4) renderBookTopics([...selected, key]); else notify('Choose up to 4 topics', true); return; }
  const recommendation = event.target.closest('[data-recommend-work]');
  if (recommendation) { openBrowseWork(state.visibleRecommendations[Number(recommendation.dataset.recommendWork)]); return; }
  if (event.target.closest('#browse-view-editions')) {
    if ($('#browse-dialog-content').hidden) {
      $('#browse-dialog-editions').hidden = true;
      $('#browse-dialog-content').hidden = false;
      $('#browse-view-editions').textContent = 'View editions';
    } else { await loadBrowseDialogEditions(); }
    return;
  }
  const homeRecommendation = event.target.closest('[data-home-recommend]');
  if (homeRecommendation) { openBrowseWork(state.visibleHomeRecommendations[Number(homeRecommendation.dataset.homeRecommend)]); return; }
  const step = event.target.closest('[data-step]'); if (step) { const id = Number(step.dataset.book); const item = state.books.find(({book}) => book.id === id); if (item?.reading) await saveInlineProgress(id, item.reading.current_value + Number(step.dataset.step)); return; }
  const progress = event.target.closest('[data-progress]'); if (progress) { try { state.selected = await library.get(Number(progress.dataset.progress)); openProgress(); } catch(e) { notify(e.message,true); } return; }
  const editProgress = event.target.closest('[data-edit-progress]');
  if (editProgress) {
    const entry = state.selected.readings.flatMap(session => session.progress).find(entry => entry.id === Number(editProgress.dataset.editProgress));
    if (entry) openProgress(entry);
    return;
  }
  const removeReading = event.target.closest('[data-remove-reading]');
  if (removeReading) {
    if (removeReading.disabled) return;
    if (!await confirmAction('Remove this reading and its progress? The book, rating and earlier readings will be kept.')) return;
    removeReading.disabled = true;
    try {
      const bookId = state.selected.book.id;
      await library.removeReading(Number(removeReading.dataset.removeReading));
      state.selected = await library.get(bookId);
      renderDetail();
      await refresh();
      notify('Reading removed');
    } catch (error) { removeReading.disabled = false; notify(error.message, true); }
    return;
  }
  const startBook = event.target.closest('[data-start-book]');
  if (startBook) {
    const item = state.books.find(({book}) => book.id === Number(startBook.dataset.startBook));
    if (!item || item.book.status !== 'want' || startBook.disabled) return;
    startBook.disabled = true;
    try {
      await library.start(item.book.id, item.book.page_count ? 'pages' : 'percent');
      await refresh();
      notify('Reading started');
    } catch (error) { startBook.disabled = false; notify(error.message, true); }
    return;
  }
  const open = event.target.closest('[data-open]'); if (open && !event.target.closest('.planned-buy')) { try { await openDetail(Number(open.dataset.open)); } catch(e) { notify(e.message,true); } return; }
  const edition = event.target.closest('[data-edition]'); if (edition) { importEdition(Number(edition.dataset.edition)); return; }
  const action = event.target.closest('[data-action]');
  if (action) {
    const name = action.dataset.action;
    if (name === 'manual') { openBookForm(); return; }
    if (name === 'edit-book') { const b = state.selected.book; $('#detail-dialog').close(); openBookForm(b); return; }
    if (name === 'delete-book') { const b = state.selected.book; if (!await confirmAction(`Remove “${b.title}” and all its reading history?`)) return; try { await library.remove(b.id); $('#detail-dialog').close(); await refresh(); notify('Book removed'); } catch(e) { notify(e.message,true); } return; }
    if (name === 'start') { try { await library.start(state.selected.book.id, $('#unit-select').value); state.selected = await library.get(state.selected.book.id); renderDetail(); await refresh(); notify('Reading started'); } catch(e) { notify(e.message,true); } return; }
    if (name === 'correct-progress') {
      const {reading, progress} = state.selected.readings[0];
      const total = reading.unit === 'percent' ? 100 : state.selected.book.page_count;
      openProgress((reading.status === 'completed' ? progress.find(entry => entry.value === total) : progress[0]) || null);
      return;
    }
    if (name === 'progress') { openProgress(); return; }
  }
  const status = event.target.closest('[data-status]'); if (status) { try { await library.status(state.selected.readings[0].reading.id, status.dataset.status); state.selected = await library.get(state.selected.book.id); renderDetail(); await refresh(); notify('Status updated'); } catch(e) { notify(e.message,true); } return; }
});
document.addEventListener('change', async event => {
  if (event.target.matches('[data-edit-reading-status]')) {
    const input = event.target;
    const previous = state.selected.readings[0].reading.status;
    input.disabled = true;
    try {
      const bookId = state.selected.book.id;
      await library.status(Number(input.dataset.editReadingStatus), input.value);
      state.selected = await library.get(bookId);
      renderDetail();
      await refresh();
      notify('Status updated');
    } catch (error) { input.value = previous; input.disabled = false; notify(error.message, true); }
    return;
  }

  if (event.target.id === 'detail-rating') {
    const input = event.target;
    const oldRating = state.selected.book.rating;
    input.disabled = true;
    try {
      const rating = input.value ? Number(input.value) : null;
      state.selected.book = await library.rate(state.selected.book.id, rating);
      await refresh();
      renderDetail();
      notify('Rating saved');
    } catch (error) {
      input.value = oldRating ?? '';
      input.disabled = false;
      notify(error.message, true);
    }
    return;
  }
  const input = event.target.closest('[data-page-input]');
  if (!input) return;
  if (input.value.trim() === '') { notify('Enter your current page', true); renderDashboard(); return; }
  saveInlineProgress(Number(input.dataset.pageInput), Number(input.value));
});
document.addEventListener('keydown', e => { if ((e.key === 'Enter' || e.key === ' ') && e.target.matches('.book-row')) { e.preventDefault(); e.target.click(); } });
$('#book-form').addEventListener('submit', async e => { e.preventDefault(); const payload = bookPayload(e.currentTarget); try { const b = await (state.bookEdit ? library.update(state.bookEdit.id, payload) : library.create(payload)); $('#book-dialog').close(); await refresh(); notify('Book saved'); openDetail(b.id); } catch(err) { notify(err.message,true); } });
$('#page-count-form').addEventListener('submit', e => { e.preventDefault(); if (!state.pendingEdition) return; const count = Number(e.currentTarget.elements.page_count.value); if (!Number.isInteger(count) || count < 1) return; createImportedBook({...state.pendingEdition, page_count: count}); });
$('#skip-pages').addEventListener('click', () => { if (state.pendingEdition) createImportedBook({...state.pendingEdition, page_count: null}); });
$('#progress-form').addEventListener('submit', async e => { e.preventDefault(); const data = Object.fromEntries(new FormData(e.currentTarget)); const payload = {value:Number(data.value),note:data.note,recorded_at:data.recorded_at}; const rid = state.progressEdit?.reading_id ?? state.selected.readings[0].reading.id; try { await (state.progressEdit ? library.editProgress(state.progressEdit.id, payload) : library.addProgress(rid, payload)); $('#progress-dialog').close(); state.selected = await library.get(state.selected.book.id); await refresh(); renderDetail(); openDialog($('#detail-dialog')); notify('Progress saved'); } catch(err) { notify(err.message,true); } });
$('#library-search').addEventListener('input',renderLibrary); $('#library-sort').addEventListener('change',renderLibrary);
$('#library-filters').addEventListener('click',e => { const b=e.target.closest('[data-filter]'); if(!b)return; state.filter=b.dataset.filter; $$('[data-filter]').forEach(x=>x.classList.toggle('active',x===b)); renderLibrary(); });
$('#filter-toggle').addEventListener('click', () => {
  const toggle = $('#filter-toggle');
  toggle.setAttribute('aria-expanded', toggle.getAttribute('aria-expanded') === 'true' ? 'false' : 'true');
  syncBrowseFilters();
});
$('#recommend-topic-search').addEventListener('input', () => filterTopicChoices($('#recommend-topic-search'), $('#recommend-genres')));
$('#book-topic-search').addEventListener('input', () => filterTopicChoices($('#book-topic-search'), $('#book-topic-choices')));
$('#recommend-load').addEventListener('click', () => {
  $('#filter-toggle').setAttribute('aria-expanded', 'false');
  syncBrowseFilters();
  loadRecommendations();
});
$('#browse-search').addEventListener('input', () => {
  clearTimeout(browseSearchTimer);
  recommendationSerial++;
  showBrowseResults();
  state.recommendations = [];
  renderRecommendations();
  const term = $('#browse-search').value.trim();
  if (term) $('#filter-toggle').setAttribute('aria-expanded', 'false');
  syncBrowseFilters();
  $('#recommend-message').textContent = term.length === 1 ? 'Enter at least two characters to search.' : term ? 'Searching books…' : 'Finding books…';
  if (term.length !== 1) browseSearchTimer = setTimeout(loadRecommendations, term ? 400 : 0);
});
$('#browse-search').addEventListener('keydown', event => {
  if (event.key !== 'Enter') return;
  event.preventDefault();
  clearTimeout(browseSearchTimer);
  loadRecommendations();
});
$('#isbn-form').addEventListener('submit', event => { event.preventDefault(); clearTimeout(browseSearchTimer); lookupIsbn($('#isbn-query').value.trim()); });
$('#close-editions').addEventListener('click',showBrowseResults);
$('#browse-dialog-more-editions').addEventListener('click', () => loadBrowseDialogEditions(true));
$('#browse-dialog').addEventListener('close', () => { browseDialogSerial++; browseDialogEditionSerial++; });
$('#more-editions').addEventListener('click',()=>showEditions(0,true));
$('#manual-add').addEventListener('click',()=>openBookForm());
$('[data-nav="browse"]').addEventListener('click',()=>{ if (state.view === 'browse') showBrowseResults(); });
$('#backup-open').addEventListener('click',()=>openDialog($('#data-dialog')));
$('#export-button').addEventListener('click', async () => {
  try {
    if (await saveBackup(await library.export(), `booklib-${today()}.json`)) notify('Backup saved');
  } catch (error) { notify(error.message, true); }
});
$('#import-button').addEventListener('click', async () => {
  try {
    const data = await loadBackup();
    if (!data) return;
    validateBackup(data);
    if (!await confirmAction(`Restore ${data.books.length} books? Your current data will be replaced.`)) return;
    await library.import(data);
    $('#data-dialog').close();
    await refresh();
    notify('Library restored');
  } catch (error) { notify(error.message, true); }
});
installExternalLinks(error => notify(error.message, true));
installBackButton(error => notify(error.message, true)).catch(error => notify(error.message, true));
document.addEventListener('error', event => {
  if (event.target.matches?.('img')) {
    event.target.hidden = true;
    if (event.target.nextElementSibling) event.target.nextElementSibling.hidden = false;
  }
}, true);
window.addEventListener('hashchange',navigate); initRecommendations(); navigate(); refresh().catch(e=>notify(e.message,true));
