CREATE TABLE IF NOT EXISTS books (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL,
  authors TEXT NOT NULL DEFAULT '',
  isbn TEXT,
  work_id TEXT,
  edition_id TEXT,
  cover_url TEXT,
  page_count INTEGER,
  language TEXT,
  published TEXT,
  description TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT 'manual',
  status TEXT NOT NULL DEFAULT 'want' CHECK (status IN ('want','reading','paused','completed','abandoned')),
  rating INTEGER CHECK (rating BETWEEN 1 AND 5),
  review TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  tags TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS books_edition_unique ON books(edition_id) WHERE edition_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS books_title_idx ON books(title);

CREATE TABLE IF NOT EXISTS readings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  book_id INTEGER NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('reading','paused','completed','abandoned')),
  unit TEXT NOT NULL CHECK (unit IN ('pages','percent')),
  current_value INTEGER NOT NULL DEFAULT 0 CHECK (current_value >= 0),
  started_at TEXT NOT NULL DEFAULT (date('now')),
  finished_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS readings_book_idx ON readings(book_id, id DESC);

CREATE TABLE IF NOT EXISTS progress_entries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  reading_id INTEGER NOT NULL REFERENCES readings(id) ON DELETE CASCADE,
  value INTEGER NOT NULL CHECK (value >= 0),
  note TEXT NOT NULL DEFAULT '',
  recorded_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS progress_reading_idx ON progress_entries(reading_id, recorded_at, id);
