ALTER TABLE books ADD COLUMN user_rating INTEGER CHECK (user_rating BETWEEN 1 AND 10);
UPDATE books SET user_rating = rating * 2 WHERE rating IS NOT NULL;
