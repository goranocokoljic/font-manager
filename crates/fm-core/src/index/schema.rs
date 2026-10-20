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
///   case-insensitively: the base of typo-tolerant search. Search only asks
///   for single trigrams, so the table keeps no positions (`detail = 'none'`)
///   and no column sizes (`columnsize = 0`): a 10k-font scan writes far less.
/// - `face_fts_source` is what a row holds: the grouped family and style, the
///   PostScript name, foundry and designer, the tags, and the file name
///   (`NULL` for a path that is not UTF-8).
/// - Triggers keep it in step with `face`, `face_tag` and `tag`, so every
///   write path (scan, tagging, rebuild) updates search without knowing it.
///   A face written again with the same names leaves its row alone.
/// - `face.foundry` and `face.designer` are columns, written with the face,
///   so a trigger never parses `face.parsed`; older rows are filled from it.
/// - `face.added` is when the face was first indexed (RFC 3339, UTC), written
///   on insert only; faces indexed before this version get the migration time.
const V3: &str = r"
ALTER TABLE face ADD COLUMN added TEXT;
ALTER TABLE face ADD COLUMN foundry TEXT;
ALTER TABLE face ADD COLUMN designer TEXT;
UPDATE face SET
    added = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
    foundry = CASE WHEN json_valid(parsed) THEN json_extract(parsed, '$.foundry') END,
    designer = CASE WHEN json_valid(parsed) THEN json_extract(parsed, '$.designer') END;

CREATE VIRTUAL TABLE face_fts USING fts5 (
    family, style, postscript_name, foundry, designer, tags, file_name,
    tokenize = 'trigram', detail = 'none', columnsize = 0
);

CREATE VIEW face_fts_source AS
SELECT
    face.id,
    face.family,
    face.subfamily,
    face.postscript_name,
    face.foundry,
    face.designer,
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
    INSERT INTO face_fts (rowid, family, style, postscript_name, foundry, designer, tags, file_name)
    SELECT * FROM face_fts_source WHERE id = new.id;
END;

CREATE TRIGGER face_fts_update
AFTER UPDATE OF family, subfamily, postscript_name, foundry, designer ON face
WHEN old.family IS NOT new.family OR old.subfamily IS NOT new.subfamily
  OR old.postscript_name IS NOT new.postscript_name
  OR old.foundry IS NOT new.foundry OR old.designer IS NOT new.designer
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

/// Search filters (E3.4): what the color and health filters read, as
/// columns written with the face, so a search never parses `face.parsed`.
///
/// - `face.is_color`: the face has color glyphs (`ParsedFace::color`). Faces
///   indexed before this version are read again by the next scan (their
///   `face.parsed` lacks `color`), which fills it.
/// - `face.healthy`: the face's own health is `Ok`; older rows are filled
///   from `face.parsed`.
const V4: &str = r"
ALTER TABLE face ADD COLUMN is_color INTEGER NOT NULL DEFAULT 0 CHECK (is_color IN (0, 1));
ALTER TABLE face ADD COLUMN healthy INTEGER NOT NULL DEFAULT 1 CHECK (healthy IN (0, 1));
UPDATE face SET healthy = 0
WHERE json_valid(parsed) AND json_extract(parsed, '$.health.kind') <> 'ok';
";

/// Coverage at scan time (E3.7).
///
/// - `face.cmap`: the code points `cmap` maps, as a `CodePointSet` (one
///   256-bit block per used Unicode page). It replaces `face.coverage`.
/// - `face.languages`: how many base characters of each language preset
///   (`data/charsets`) the face maps, and whether it has `locl` forms for the
///   language, as one JSON object per face: `{"ro": [60, 60, 1]}` is
///   (covered, total, has `locl`). A preset with nothing covered and no
///   `locl` is left out. One value per face, not one row per language: a
///   10k-font scan would otherwise write some 350,000 rows.
/// - `index_meta`: `charsets` holds the fingerprint of the preset data
///   `face.languages` was computed with; on open, other data computes it again.
///
/// Faces indexed before this version have no `cmap` and their `face.parsed`
/// lacks `loclLanguages`, so the next scan reads their files again.
const V5: &str = r"
ALTER TABLE face ADD COLUMN cmap BLOB;
ALTER TABLE face ADD COLUMN languages TEXT;
ALTER TABLE face DROP COLUMN coverage;

CREATE TABLE index_meta (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
) WITHOUT ROWID;
";

/// Collections (E7.1): `collection.position` orders the children of one
/// parent, 0 first. Collections made before this version keep their id order.
const V6: &str = r"
ALTER TABLE collection ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
UPDATE collection SET position = id;
";

/// Sidecar metadata (E7.3, `index/sidecars.rs`).
///
/// - `sidecar_dirty`: faces whose user metadata changed since their family's
///   sidecar was last written. Triggers fill it from every write to
///   `face_tag`, `user_meta` and `collection_face`, so no write path can
///   forget the mirror; a renamed or moved collection marks its faces in
///   `index/collections.rs`. No foreign key: a deleted face's row is dropped
///   by the next write of the sidecars. The triggers test for the row
///   instead of `INSERT OR IGNORE`: an outer upsert's conflict handling
///   would override theirs and fail on a face marked twice.
/// - `sidecar_base`: each sidecar file as this machine last wrote or read
///   it, the base of the three-way merge, restricted to the faces this
///   machine has indexed. `stamp` is the file's size and modification time
///   then: a scan that finds the same stamp does not read the file again.
/// - `sidecar_conflict`: a value from a sidecar that lost to this machine's
///   one, kept until the user sets the field again; `field = 'file'` (no
///   face) is a sidecar that cannot be read, kept until it can.
const V7: &str = r"
CREATE TABLE sidecar_dirty (
    face_id  INTEGER PRIMARY KEY
) WITHOUT ROWID;

CREATE TABLE sidecar_base (
    source_id  INTEGER NOT NULL REFERENCES source (id) ON DELETE CASCADE,
    file       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    stamp      TEXT,
    PRIMARY KEY (source_id, file)
) WITHOUT ROWID;

CREATE TABLE sidecar_conflict (
    id         INTEGER PRIMARY KEY,
    source_id  INTEGER NOT NULL REFERENCES source (id) ON DELETE CASCADE,
    file       TEXT    NOT NULL,
    face_id    INTEGER REFERENCES face (id) ON DELETE CASCADE,
    field      TEXT    NOT NULL,
    local      TEXT,
    disk       TEXT    NOT NULL
);
CREATE UNIQUE INDEX sidecar_conflict_value
    ON sidecar_conflict (source_id, file, coalesce(face_id, 0), field, disk);
CREATE INDEX sidecar_conflict_face ON sidecar_conflict (face_id);

CREATE TRIGGER sidecar_dirty_tag_insert AFTER INSERT ON face_tag BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT new.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = new.face_id);
END;
CREATE TRIGGER sidecar_dirty_tag_delete AFTER DELETE ON face_tag BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT old.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = old.face_id);
END;
CREATE TRIGGER sidecar_dirty_meta_insert AFTER INSERT ON user_meta BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT new.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = new.face_id);
END;
CREATE TRIGGER sidecar_dirty_meta_update AFTER UPDATE ON user_meta BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT new.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = new.face_id);
END;
CREATE TRIGGER sidecar_dirty_meta_delete AFTER DELETE ON user_meta BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT old.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = old.face_id);
END;
CREATE TRIGGER sidecar_dirty_collection_insert AFTER INSERT ON collection_face BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT new.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = new.face_id);
END;
CREATE TRIGGER sidecar_dirty_collection_delete AFTER DELETE ON collection_face BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT old.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = old.face_id);
END;
";

/// The activation service (E6.6, `activation.rs`).
///
/// - `activation_registration`: what the OS holds for an active face, as
///   `FontActivator::activate` returned it: the file it points at and the
///   activator's handle (registry value name, link, URL), both stored like
///   `font_file.path` (TEXT when UTF-8, else the native bytes as a BLOB).
///   Faces of one collection file share a handle. A row without one (none
///   are written before this version) has nothing to deregister.
/// - `last_session`: the faces that were session-active when the app last
///   quit or crashed, for "reactivate on launch". Written at quit, and at
///   the next start after a crash; a face that leaves the index leaves it.
const V8: &str = r"
CREATE TABLE activation_registration (
    face_id  INTEGER PRIMARY KEY REFERENCES activation (face_id) ON DELETE CASCADE,
    path     NOT NULL,
    handle   NOT NULL
);
CREATE INDEX activation_registration_handle ON activation_registration (handle);

CREATE TABLE last_session (
    face_id  INTEGER PRIMARY KEY REFERENCES face (id) ON DELETE CASCADE
) WITHOUT ROWID;
";

/// "Keep this one" (E8.3, `resolve.rs`): the Problems history and what undo
/// needs.
///
/// - `resolution`: one action; `kept` is the copy kept, as the user saw it.
/// - `resolution_move`: each copy of the action in request order: its
///   `path` (stored like `font_file.path`), the `display` path the API
///   shows, its `TrashMoveStatus` (camelCase serde name), a message, and
///   where the OS trash put it (`location`, `info`: the `Trashed` the
///   platform returned, stored the same way), which undo moves back.
const V9: &str = r"
CREATE TABLE resolution (
    id      INTEGER PRIMARY KEY,
    at      TEXT NOT NULL,
    kept    TEXT NOT NULL,
    undone  INTEGER NOT NULL DEFAULT 0 CHECK (undone IN (0, 1))
);

CREATE TABLE resolution_move (
    resolution_id  INTEGER NOT NULL REFERENCES resolution (id) ON DELETE CASCADE,
    position       INTEGER NOT NULL,
    path           NOT NULL,
    display        TEXT NOT NULL,
    status         TEXT NOT NULL,
    message        TEXT,
    location,
    info,
    PRIMARY KEY (resolution_id, position)
) WITHOUT ROWID;
";

/// Virtual styles (E10.2, `index/virtual_styles.rs`): an instance of a
/// variable face pinned in one manual collection or one project set.
///
/// - Exactly one of `collection_id` and `set_id` is set; deleting either
///   deletes the style, deleting the face too. The font file is never
///   touched.
/// - `coordinates`: every axis of the face as `fm-api` `AxisValue` JSON, in
///   the font's axis order.
/// - A name is unique per face and holder, in any case.
/// - The triggers mark the face dirty, so the sidecar mirror (V7) carries
///   the style; a cascade from a deleted holder fires them too.
const V10: &str = r"
CREATE TABLE virtual_style (
    id             INTEGER PRIMARY KEY,
    face_id        INTEGER NOT NULL REFERENCES face (id) ON DELETE CASCADE,
    name           TEXT    NOT NULL,
    coordinates    TEXT    NOT NULL,
    collection_id  INTEGER REFERENCES collection (id) ON DELETE CASCADE,
    set_id         INTEGER REFERENCES project_set (id) ON DELETE CASCADE,
    CHECK ((collection_id IS NULL) <> (set_id IS NULL))
);
CREATE UNIQUE INDEX virtual_style_name
    ON virtual_style (face_id, coalesce(collection_id, 0), coalesce(set_id, 0), name COLLATE NOCASE);
CREATE INDEX virtual_style_collection ON virtual_style (collection_id);
CREATE INDEX virtual_style_set ON virtual_style (set_id);

CREATE TRIGGER sidecar_dirty_virtual_style_insert AFTER INSERT ON virtual_style BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT new.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = new.face_id);
END;
CREATE TRIGGER sidecar_dirty_virtual_style_delete AFTER DELETE ON virtual_style BEGIN
    INSERT INTO sidecar_dirty (face_id) SELECT old.face_id
    WHERE NOT EXISTS (SELECT 1 FROM sidecar_dirty WHERE face_id = old.face_id);
END;
";

/// Family notes (E13.5, `index/family_notes.rs`): the user's notes on a
/// family, by its grouping key (`grouping::family_key`, the key the sidecar
/// file is named by), not by a face id, so they outlive a rescan that
/// indexes the family's faces again. A family no face has any more keeps
/// its row; the note is back when the fonts are.
const V11: &str = r"
CREATE TABLE family_meta (
    family_key  TEXT PRIMARY KEY,
    notes       TEXT
) WITHOUT ROWID;
";

/// Open licenses at scan time (E15.1, `license.rs`): `face.open_license` is
/// the open license the face's name table states, as its `fm-api` serde name
/// (`ofl`, `apache`, `ufl`, `mit`), `NULL` for none. A scan writes it with
/// the face, so search never reads `face.parsed` for it. Rows indexed before
/// this version are filled once from their `face.parsed` license text when
/// the migration runs ([`OPEN_LICENSE_VERSION`]).
const V12: &str = r"
ALTER TABLE face ADD COLUMN open_license TEXT;
";

/// The schema version that adds `face.open_license`; the migration to it
/// also fills the column for the faces already indexed.
pub(crate) const OPEN_LICENSE_VERSION: u32 = 12;

/// Every migration, in order. The schema version is the length of this list.
pub(crate) const MIGRATIONS: &[&str] = &[V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12];

/// The schema version this build writes.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// The tables of the data model, sorted by name.
#[cfg(test)]
pub(crate) const TABLES: &[&str] = &[
    "activation",
    "activation_registration",
    "collection",
    "collection_face",
    "face",
    "face_fts",
    "face_fts_config",
    "face_fts_content",
    "face_fts_data",
    "face_fts_idx",
    "face_tag",
    "family_meta",
    "font_file",
    "index_meta",
    "last_session",
    "project_set",
    "resolution",
    "resolution_move",
    "set_face",
    "sidecar_base",
    "sidecar_conflict",
    "sidecar_dirty",
    "source",
    "tag",
    "user_meta",
    "virtual_style",
];
