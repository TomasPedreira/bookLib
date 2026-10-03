-- Convert legacy readings only when the copy's real page count is known.
-- Otherwise retain the original values until the user supplies that count.
UPDATE progress_entries
SET value = CAST(round(value * (SELECT b.page_count FROM readings r JOIN books b ON b.id=r.book_id WHERE r.id=progress_entries.reading_id) / 100.0) AS INTEGER)
WHERE reading_id IN (SELECT r.id FROM readings r JOIN books b ON b.id=r.book_id WHERE r.unit='percent' AND b.page_count IS NOT NULL);

UPDATE readings
SET current_value = CASE WHEN status='completed' THEN (SELECT page_count FROM books WHERE id=readings.book_id)
    ELSE CAST(round(current_value * (SELECT page_count FROM books WHERE id=readings.book_id) / 100.0) AS INTEGER) END,
    unit='pages'
WHERE unit='percent' AND (SELECT page_count FROM books WHERE id=readings.book_id) IS NOT NULL;
