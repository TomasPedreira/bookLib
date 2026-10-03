import { invoke } from '@tauri-apps/api/core';
import { apiErrorMessage } from './errors.js';

export function createServices(call) {
  async function command(name, args = {}) {
    try { return await call(name, args); }
    catch (error) { throw new Error(apiErrorMessage(error?.message || String(error), error?.code)); }
  }
  return {
    library: {
      list: () => command('list_books'),
      get: id => command('get_book', { id }),
      create: b => command('create_book', { b }),
      update: (id, b) => command('update_book', { id, b }),
      rate: (id, rating) => command('update_rating', { id, input: { rating } }),
      remove: id => command('delete_book', { id }),
      start: (id, unit) => command('start_reading', { id, input: { unit } }),
      removeReading: id => command('delete_reading', { id }),
      status: (id, status) => command('change_reading', { id, input: { status } }),
      addProgress: (id, input) => command('add_progress', { id, input }),
      editProgress: (id, input) => command('edit_progress', { id, input }),
      editDay: (id, input) => command('edit_progress_day', { id, input }),
      deleteProgress: id => command('delete_progress', { id }),
      stats: () => command('stats'),
      export: () => command('export_data'),
      import: data => command('import_data', { data }),
    },
    catalog: {
      search: q => command('search', { query: { q } }),
      isbn: input => command('lookup_isbn', { input }),
      recommendations: query => command('recommendations', { query }),
      details: id => command('work_details', { id }),
      genres: id => command('work_genres', { id }),
      editions: (id, offset = 0) => command('editions', { id, query: { offset } }),
    },
  };
}

export const { library, catalog } = createServices(invoke);
