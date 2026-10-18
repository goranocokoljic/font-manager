//! The database schema and its migrations.
//!
//! The schema version is SQLite's `user_version`. Migration `n` (1-based)
//! takes the database from version `n - 1` to `n`. A migration is never edited
//! once it has shipped; a change to the schema is a new migration at the end.

/// The data model of the spec (ADR 0003): `source` → `font_file` → `face`,
/// plus collections, tags, project sets, user metadata and activation.
///
/// - `font_file.hash` and `font_file.mtime` are the read stamp. `NULL` means
///   "not read yet": the next scan reads the file again (see `Index::rebuild`).
/// - JSON columns hold the `fm-api` types as serde writes them.
/// - The project-set table is `project_set`: `SET` is an SQL keyword.
/// - `activation.face_id` is `RESTRICT`: a face the app registered cannot leave
///   the index while it is still active (rule 3, never lose what we activated).
const V1: &str = "
CREATE TABLE source (
    id      INTEGER PRIMARY KEY,
    path    TEXT    NOT NULL UNIQUE,
    kind    TEXT    NOT NULL CHECK (kind IN ('watched', 'system', 'libraryFolder')),
    paused  INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0, 1))
);

CREATE TABLE font_file (
    id         INTEGER PRIMARY KEY,
    source_id  INTEGER NOT NULL REFERENCES source (id) ON DELETE CASCADE,
    path       TEXT    NOT NULL UNIQUE,
    hash       BLOB,
    format     TEXT    NOT NULL,
    size       INTEGER NOT NULL,
    mtime      INTEGER,
    health     TEXT    NOT NULL
);
CREATE INDEX font_file_source ON font_file (source_id);
CREATE INDEX font_file_hash ON font_file (hash);

CREATE TABLE face (
    id                  INTEGER PRIMARY KEY,
    file_id             INTEGER NOT NULL REFERENCES font_file (id) ON DELETE CASCADE,
    collection_index    INTEGER NOT NULL DEFAULT 0,
    family              TEXT    NOT NULL,
    subfamily           TEXT    NOT NULL,
    typographic_family  TEXT,
    postscript_name     TEXT    NOT NULL,
    weight              INTEGER NOT NULL,
    width               INTEGER NOT NULL,
    slant               TEXT    NOT NULL,
    is_variable         INTEGER NOT NULL DEFAULT 0 CHECK (is_variable IN (0, 1)),
    axes                TEXT    NOT NULL DEFAULT '[]',
    features            TEXT    NOT NULL DEFAULT '[]',
    scripts             TEXT    NOT NULL DEFAULT '[]',
    metrics             TEXT,
    fstype              INTEGER NOT NULL DEFAULT 0,
    UNIQUE (file_id, collection_index)
);
CREATE INDEX face_family ON face (family);
CREATE INDEX face_typographic_family ON face (typographic_family);
CREATE INDEX face_postscript_name ON face (postscript_name);

CREATE TABLE collection (
    id         INTEGER PRIMARY KEY,
    parent_id  INTEGER REFERENCES collection (id) ON DELETE CASCADE,
    name       TEXT    NOT NULL,
    is_smart   INTEGER NOT NULL DEFAULT 0 CHECK (is_smart IN (0, 1)),
    rules      TEXT
);
CREATE INDEX collection_parent ON collection (parent_id);

CREATE TABLE collection_face (
    collection_id  INTEGER NOT NULL REFERENCES collection (id) ON DELETE CASCADE,
    face_id        INTEGER NOT NULL REFERENCES face (id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, face_id)
) WITHOUT ROWID;
CREATE INDEX collection_face_face ON collection_face (face_id);

CREATE TABLE tag (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE face_tag (
    face_id  INTEGER NOT NULL REFERENCES face (id) ON DELETE CASCADE,
    tag_id   INTEGER NOT NULL REFERENCES tag (id) ON DELETE CASCADE,
    PRIMARY KEY (face_id, tag_id)
) WITHOUT ROWID;
CREATE INDEX face_tag_tag ON face_tag (tag_id);

CREATE TABLE project_set (
    id    INTEGER PRIMARY KEY,
    name  TEXT NOT NULL
);

CREATE TABLE set_face (
    set_id   INTEGER NOT NULL REFERENCES project_set (id) ON DELETE CASCADE,
    face_id  INTEGER NOT NULL REFERENCES face (id) ON DELETE CASCADE,
    PRIMARY KEY (set_id, face_id)
) WITHOUT ROWID;
CREATE INDEX set_face_face ON set_face (face_id);

CREATE TABLE user_meta (
    face_id       INTEGER PRIMARY KEY REFERENCES face (id) ON DELETE CASCADE,
    rating        INTEGER CHECK (rating BETWEEN 1 AND 5),
    favorite      INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
    license_type  TEXT,
    seats         INTEGER CHECK (seats >= 0),
    notes         TEXT
);

CREATE TABLE activation (
    face_id   INTEGER PRIMARY KEY REFERENCES face (id) ON DELETE RESTRICT,
    mode      TEXT NOT NULL CHECK (mode IN ('session', 'permanent')),
    document  TEXT,
    since     TEXT NOT NULL
);
";

/// What a scan needs to load its file index back (E3.2).
///
/// - `font_file.gone`: the last rescan did not find the file; it stays with
///   its faces and user data (ADR 0004).
/// - `face.parsed`: the whole parsed face as JSON, so a loaded index equals
///   the one that was saved. `NULL` or unreadable JSON means "read again".
/// - `face.coverage`: the code points as packed little-endian `u32` pairs
///   (first, last) of each run, a fraction of one row per code point.
const V2: &str = "
ALTER TABLE font_file ADD COLUMN gone INTEGER NOT NULL DEFAULT 0 CHECK (gone IN (0, 1));
ALTER TABLE face ADD COLUMN parsed TEXT;
ALTER TABLE face ADD COLUMN coverage BLOB;
";

/// Text search (E3.3): one FTS5 row per face, rowid = `face.id`.
///
/// - The trigram tokenizer matches any three characters anywhere in a name,
///   case-insensitively: the base of typo-tolerant search.
/// - `face_fts_source` is what a row holds: the grouped family and style, the
///   PostScript name, foundry and designer from the parsed face, the tags,
///   and the file name (`NULL` for a path that is not UTF-8).
/// - Triggers keep it in step with `face`, `face_tag` and `tag`, so every
///   write path (scan, tagging, rebuild) updates search without knowing it.
/// - `face.added` is when the face was first indexed (RFC 3339, UTC); faces
///   indexed before this version get the time of the migration.
const V3: &str = r"
ALTER TABLE face ADD COLUMN added TEXT;
UPDATE face SET added = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');

CREATE VIRTUAL TABLE face_fts USING fts5 (
    family, style, postscript_name, foundry, designer, tags, file_name,
    tokenize = 'trigram'
);

CREATE VIEW face_fts_source AS
SELECT
    face.id,
    face.family,
    face.subfamily,
    face.postscript_name,
    CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.foundry') END,
    CASE WHEN json_valid(face.parsed) THEN json_extract(face.parsed, '$.designer') END,
    (SELECT group_concat(tag.name, ' ') FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
     WHERE face_tag.face_id = face.id),
    CASE WHEN typeof(font_file.path) = 'text' THEN substr(
        font_file.path,
        length(rtrim(font_file.path, replace(replace(font_file.path, '/', ''), '\', ''))) + 1
    ) END
FROM face JOIN font_file ON font_file.id = face.file_id;

INSERT INTO face_fts (rowid, family, style, postscript_name, foundry, designer, tags, file_name)
SELECT * FROM face_fts_source;

CREATE TRIGGER face_fts_insert AFTER INSERT ON face BEGIN
    UPDATE face SET added = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = new.id;
    INSERT INTO face_fts (rowid, family, style, postscript_name, foundry, designer, tags, file_name)
    SELECT * FROM face_fts_source WHERE id = new.id;
END;

CREATE TRIGGER face_fts_update AFTER UPDATE OF family, subfamily, postscript_name, parsed ON face
BEGIN
    DELETE FROM face_fts WHERE rowid = old.id;
    INSERT INTO face_fts (rowid, family, style, postscript_name, foundry, designer, tags, file_name)
    SELECT * FROM face_fts_source WHERE id = new.id;
END;

CREATE TRIGGER face_fts_delete AFTER DELETE ON face BEGIN
    DELETE FROM face_fts WHERE rowid = old.id;
END;

CREATE TRIGGER face_fts_tag_insert AFTER INSERT ON face_tag BEGIN
    UPDATE face_fts SET tags = (SELECT group_concat(tag.name, ' ') FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
         WHERE face_tag.face_id = new.face_id)
    WHERE rowid = new.face_id;
END;

CREATE TRIGGER face_fts_tag_delete AFTER DELETE ON face_tag BEGIN
    UPDATE face_fts SET tags = (SELECT group_concat(tag.name, ' ') FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
         WHERE face_tag.face_id = old.face_id)
    WHERE rowid = old.face_id;
END;

CREATE TRIGGER face_fts_tag_rename AFTER UPDATE OF name ON tag BEGIN
    UPDATE face_fts SET tags = (SELECT group_concat(tag.name, ' ') FROM face_tag JOIN tag ON tag.id = face_tag.tag_id
         WHERE face_tag.face_id = face_fts.rowid)
    WHERE rowid IN (SELECT face_id FROM face_tag WHERE tag_id = new.id);
END;
";

/// Every migration, in order. The schema version is the length of this list.
pub(crate) const MIGRATIONS: &[&str] = &[V1, V2, V3];

/// The schema version this build writes.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// The tables of the data model, sorted by name.
#[cfg(test)]
pub(crate) const TABLES: &[&str] = &[
    "activation",
    "collection",
    "collection_face",
    "face",
    "face_fts",
    "face_fts_config",
    "face_fts_content",
    "face_fts_data",
    "face_fts_docsize",
    "face_fts_idx",
    "face_tag",
    "font_file",
    "project_set",
    "set_face",
    "source",
    "tag",
    "user_meta",
];
