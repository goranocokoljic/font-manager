//! Foundries of the library (E50.11): [`Index::list_foundries`], the
//! user's merges and the `foundries` search filter.
//!
//! The search cache knows each searchable face's foundry
//! ([`crate::foundries`]), merges applied; a merge is a row of
//! `foundry_merge`, so writing one moves the cache's stamp and the next read
//! groups again.

use std::collections::{BTreeSet, HashSet};

use fm_api::FaceId;
use fm_api::search::{FoundryInfo, MergeFoundriesRequest, UNKNOWN_FOUNDRY, UnmergeFoundryRequest};
use rusqlite::params;

use crate::error::{FmError, FmResult};
use crate::foundries::{Merge, Table, group_key};
use crate::index::{Index, db_error};

/// The foundries a search asks for, as slots of the cache's table.
pub(super) struct Wanted {
    slots: HashSet<u32>,
    unknown: bool,
}

impl Wanted {
    /// The slots `names` name; a name no face lists under finds nothing.
    pub(super) fn new(table: &Table, names: &[String]) -> Self {
        let unknown_key = group_key(UNKNOWN_FOUNDRY);
        Self {
            slots: names.iter().filter_map(|n| table.find(n)).collect(),
            unknown: names.iter().any(|n| group_key(n) == unknown_key),
        }
    }

    /// Whether a face of `slot` is one of them.
    pub(super) fn holds(&self, slot: Option<u32>) -> bool {
        match slot {
            Some(slot) => self.slots.contains(&slot),
            None => self.unknown,
        }
    }
}

impl Index {
    /// The user's foundry merges, as stored.
    pub(super) fn foundry_merges(&self) -> FmResult<Vec<Merge>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT from_name, into_name FROM foundry_merge ORDER BY from_key")
            .map_err(|e| db_error(&e))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Merge {
                    name: row.get(0)?,
                    into: row.get(1)?,
                })
            })
            .map_err(|e| db_error(&e))?;
        rows.collect::<Result<_, _>>().map_err(|e| db_error(&e))
    }

    /// The foundries of the searchable faces, A to Z, each with its family
    /// groups and the raw names it folds; the faces with no foundry last,
    /// as [`UNKNOWN_FOUNDRY`]. A family with faces of two foundries counts
    /// under both.
    pub fn list_foundries(&self) -> FmResult<Vec<FoundryInfo>> {
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let mut families: Vec<BTreeSet<&str>> = vec![BTreeSet::new(); cache.foundries.slots.len()];
        let mut unknown: BTreeSet<&str> = BTreeSet::new();
        for row in &cache.rows {
            match row.foundry.and_then(|s| families.get_mut(s as usize)) {
                Some(set) => set.insert(row.family_key.as_str()),
                None => unknown.insert(row.family_key.as_str()),
            };
        }
        let count = |set: &BTreeSet<&str>| u32::try_from(set.len()).unwrap_or(u32::MAX);
        let mut out: Vec<FoundryInfo> = cache
            .foundries
            .slots
            .iter()
            .zip(&families)
            .filter(|(_, set)| !set.is_empty())
            .map(|(slot, set)| FoundryInfo {
                name: slot.name.clone(),
                families: count(set),
                raw_names: slot.raws.iter().cloned().collect(),
                merged: slot.merged.iter().cloned().collect(),
                unknown: false,
            })
            .collect();
        if !unknown.is_empty() {
            out.push(FoundryInfo {
                name: UNKNOWN_FOUNDRY.to_owned(),
                families: count(&unknown),
                raw_names: Vec::new(),
                merged: Vec::new(),
                unknown: true,
            });
        }
        Ok(out)
    }

    /// The searchable faces listed under the foundry `name`, merged ones
    /// included.
    pub fn foundry_faces(&self, name: &str) -> FmResult<Vec<FaceId>> {
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        let Some(slot) = cache.foundries.find(name) else {
            return Ok(Vec::new());
        };
        Ok(cache
            .rows
            .iter()
            .filter(|row| row.foundry == Some(slot))
            .map(|row| FaceId(row.id))
            .collect())
    }

    /// Merges one listed foundry into another, as `fm_api`'s
    /// `merge_foundries` says: `NotFound` for a name not listed,
    /// `InvalidRequest` for the same name twice or the unknown foundry.
    pub fn merge_foundries(&mut self, req: &MergeFoundriesRequest) -> FmResult<Vec<FoundryInfo>> {
        let unknown = group_key(UNKNOWN_FOUNDRY);
        if group_key(&req.from) == unknown || group_key(&req.into) == unknown {
            return Err(FmError::invalid("The unknown foundry cannot be merged."));
        }
        let listed = self.list_foundries()?;
        let find = |name: &str| {
            let k = group_key(name);
            listed
                .iter()
                .find(|f| !f.unknown && group_key(&f.name) == k)
                .map(|f| f.name.clone())
                .ok_or_else(|| FmError::not_found(format!("The foundry {:?}", name.trim())))
        };
        let (from, into) = (find(&req.from)?, find(&req.into)?);
        let from_key = group_key(&from);
        // Two listed names are two keys; a merge back is a name not listed.
        if from_key == group_key(&into) {
            return Err(FmError::invalid("A foundry cannot be merged into itself."));
        }
        self.conn
            .execute(
                "INSERT INTO foundry_merge (from_key, from_name, into_name) VALUES (?1, ?2, ?3)
                 ON CONFLICT (from_key) DO UPDATE SET
                     from_name = excluded.from_name, into_name = excluded.into_name",
                params![from_key, from, into],
            )
            .map_err(|e| db_error(&e))?;
        self.list_foundries()
    }

    /// Undoes the merge of the foundry `name`; `NotFound` when it is not
    /// merged.
    pub fn unmerge_foundry(&mut self, req: &UnmergeFoundryRequest) -> FmResult<Vec<FoundryInfo>> {
        let removed = self
            .conn
            .execute(
                "DELETE FROM foundry_merge WHERE from_key = ?1",
                params![group_key(&req.name)],
            )
            .map_err(|e| db_error(&e))?;
        if removed == 0 {
            return Err(FmError::not_found(format!(
                "A merge of the foundry {:?}",
                req.name.trim()
            )));
        }
        self.list_foundries()
    }
}
