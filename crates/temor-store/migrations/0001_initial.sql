-- temor's history and the places it keeps (PLAN §2b, P23).

CREATE TABLE schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);
INSERT INTO schema_version (version, applied_at) VALUES (1, datetime('now'));

-- One shell or program, in one place, from start to end. `id` is the SessionId's 64 bits.
CREATE TABLE sessions (
    id      INTEGER PRIMARY KEY,
    host    TEXT    NOT NULL,
    folder  TEXT    NOT NULL,
    program TEXT,
    started INTEGER NOT NULL,
    ended   INTEGER
);
CREATE INDEX sessions_by_start ON sessions (started DESC);
CREATE INDEX sessions_by_folder ON sessions (folder);

-- Each command a session ran, by its block number. Never its output.
CREATE TABLE commands (
    session INTEGER NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    block   INTEGER NOT NULL,
    command TEXT    NOT NULL,
    folder  TEXT    NOT NULL,
    exit    INTEGER,
    started INTEGER NOT NULL,
    ended   INTEGER,
    PRIMARY KEY (session, block)
);
CREATE INDEX commands_by_start ON commands (started DESC);

-- Trigrams, so a search finds any part of a command or a folder, as ^R does.
CREATE VIRTUAL TABLE commands_fts USING fts5 (
    command, folder,
    content = 'commands', content_rowid = 'rowid', tokenize = 'trigram'
);
CREATE TRIGGER commands_ai AFTER INSERT ON commands BEGIN
    INSERT INTO commands_fts (rowid, command, folder) VALUES (new.rowid, new.command, new.folder);
END;
CREATE TRIGGER commands_ad AFTER DELETE ON commands BEGIN
    INSERT INTO commands_fts (commands_fts, rowid, command, folder)
        VALUES ('delete', old.rowid, old.command, old.folder);
END;
CREATE TRIGGER commands_au AFTER UPDATE OF command, folder ON commands BEGIN
    INSERT INTO commands_fts (commands_fts, rowid, command, folder)
        VALUES ('delete', old.rowid, old.command, old.folder);
    INSERT INTO commands_fts (rowid, command, folder) VALUES (new.rowid, new.command, new.folder);
END;

-- The pinned rows and the favourites, each a temor:// URL, in the order the person put them.
CREATE TABLE pins (
    place INTEGER PRIMARY KEY,
    url   TEXT NOT NULL UNIQUE
);
CREATE TABLE favourites (
    place INTEGER PRIMARY KEY,
    url   TEXT NOT NULL UNIQUE
);

-- What the window keeps between launches that is not history: its rows and Spaces, the hosts,
-- what was imported. Each value is JSON the client owns.
CREATE TABLE layout (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) WITHOUT ROWID;
