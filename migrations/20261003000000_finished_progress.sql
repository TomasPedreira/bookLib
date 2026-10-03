-- Preserve the recorded history and add the missing completion value.
INSERT INTO progress_entries (reading_id, value, recorded_at)
SELECT r.id, CASE WHEN r.unit = 'percent' THEN 100 ELSE b.page_count END,
       max(COALESCE(r.finished_at, date('now')),
           COALESCE((SELECT max(substr(p.recorded_at, 1, 10)) FROM progress_entries p WHERE p.reading_id = r.id), '0001-01-01'))
FROM readings r JOIN books b ON b.id = r.book_id
WHERE r.status = 'completed' AND (r.unit = 'percent' OR b.page_count IS NOT NULL)
  AND COALESCE((SELECT p.value FROM progress_entries p WHERE p.reading_id = r.id
                ORDER BY substr(p.recorded_at, 1, 10) DESC, p.id DESC LIMIT 1), -1)
      != CASE WHEN r.unit = 'percent' THEN 100 ELSE b.page_count END;

UPDATE readings SET current_value = CASE WHEN unit = 'percent' THEN 100
    ELSE (SELECT page_count FROM books WHERE id = readings.book_id) END
WHERE status = 'completed' AND (unit = 'percent' OR
    (SELECT page_count FROM books WHERE id = readings.book_id) IS NOT NULL);
