ALTER TABLE checks ADD COLUMN created_at TEXT NOT NULL DEFAULT '';

UPDATE checks
SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 946684800 + position, 'unixepoch')
WHERE created_at = '';

CREATE TRIGGER IF NOT EXISTS checks_created_at_default
AFTER INSERT ON checks
WHEN NEW.created_at = ''
BEGIN
    UPDATE checks
    SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    WHERE id = NEW.id;
END;
