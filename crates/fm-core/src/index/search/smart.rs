//! Smart collections (E14.2): a collection's rule, compiled into a test
//! the search rows pass or fail.
//!
//! Membership is never stored. Every search and every collection list
//! compiles the rules as they are now and runs them over the searchable rows
//! ([`super::SearchCache`]), which are read again whenever the database
//! changes: a scan, a tag, a rating, a license or an activation shows at once.
//!
//! A rule's filters may name another collection: a manual one by its members,
//! a smart one by its own rule. A rule that leads back to a collection it
//! started from is `InvalidRequest`; one that names a collection that no
//! longer exists holds nothing.

use std::collections::HashMap;

use fm_api::CollectionId;
use fm_api::organization::{Rule, RuleGroup, RuleOp};
use fm_api::search::{Filters, Supports};
use rusqlite::Connection;

use super::{Index, Row};
use crate::coverage::Need;
use crate::error::{FmError, FmResult};
use crate::filter::{failed_activated, failed_added, passes};
use crate::index::db_error;
use crate::parallel::map_ordered;

/// Rows per piece of work when the smart collections are counted on every
/// core: small enough that a 10,000-face library makes enough pieces to
/// spread ([`map_ordered`] runs a handful of pieces on one thread).
const COUNT_CHUNK: usize = 256;

/// Every collection by id, with its rule; `None` for a manual one.
pub(in crate::index) type Rules = HashMap<CollectionId, (String, Option<RuleGroup>)>;

/// A stored rule: a [`RuleGroup`], or plain filters as phase 1 stored them
/// (read as one AND group).
pub(in crate::index) fn stored_rule(json: &str) -> serde_json::Result<RuleGroup> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    if value.get("op").is_some() {
        serde_json::from_value(value)
    } else {
        serde_json::from_value::<Filters>(value).map(RuleGroup::from)
    }
}

/// Every collection and its rule.
pub(in crate::index) fn load_rules(conn: &Connection) -> FmResult<Rules> {
    let mut stmt = conn
        .prepare_cached("SELECT id, name, rules FROM collection")
        .map_err(|e| db_error(&e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| db_error(&e))?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, name, rules) = row.map_err(|e| db_error(&e))?;
        let rule = rules
            .map(|json| {
                stored_rule(&json).map_err(|e| {
                    FmError::internal(format!("collection {id} has unreadable rules: {e}"))
                })
            })
            .transpose()?;
        out.insert(CollectionId(id), (name, rule));
    }
    Ok(out)
}

/// Who is in a collection.
pub(in crate::index) enum Member {
    /// The faces added to a manual collection.
    Manual(CollectionId),
    /// The faces that pass a smart collection's rule.
    Smart(Group),
    /// Nobody: the collection does not exist (any more).
    Nothing,
    /// A smart collection whose rule already ran on this row: its result is
    /// at this position of the `done` results [`Member::holds`] is given.
    Known(usize),
}

impl Member {
    /// Compiles the test for the collection `id`. `conn` gives the time
    /// for `added_within_days` and `activated_within_days`.
    pub(in crate::index) fn compile(
        conn: &Connection,
        rules: &Rules,
        id: CollectionId,
    ) -> FmResult<Self> {
        Self::compile_with(conn, rules, &HashMap::new(), id)
    }

    /// [`Member::compile`], taking the result of each smart collection in
    /// `known` from its position in `done` (see [`Member::holds`]) instead
    /// of running its rule again.
    fn compile_with(
        conn: &Connection,
        rules: &Rules,
        known: &HashMap<CollectionId, usize>,
        id: CollectionId,
    ) -> FmResult<Self> {
        Compiler {
            conn,
            rules,
            known,
            path: Vec::new(),
        }
        .member(id)
    }

    /// Whether `row` is in the collection. `done` holds the results, for
    /// this row, of the collections compiled as [`Member::Known`]; empty for
    /// a member compiled with [`Member::compile`].
    pub(super) fn holds(&self, row: &Row, done: &[bool]) -> bool {
        match self {
            Self::Manual(id) => row.facts.collections.contains(id),
            Self::Smart(group) => group.passes(row, done),
            Self::Nothing => false,
            Self::Known(at) => done.get(*at).copied().unwrap_or(false),
        }
    }
}

/// A compiled [`RuleGroup`].
pub(in crate::index) struct Group {
    op: RuleOp,
    rules: Vec<Compiled>,
}

impl Group {
    fn passes(&self, row: &Row, done: &[bool]) -> bool {
        match self.op {
            RuleOp::And => self.rules.iter().all(|r| r.passes(row, done)),
            // An empty group passes every face, whatever its operator.
            RuleOp::Or => self.rules.is_empty() || self.rules.iter().any(|r| r.passes(row, done)),
        }
    }
}

enum Compiled {
    Leaf(Box<Leaf>),
    Group(Group),
}

impl Compiled {
    fn passes(&self, row: &Row, done: &[bool]) -> bool {
        match self {
            Self::Leaf(leaf) => leaf.passes(row, done),
            Self::Group(group) => group.passes(row, done),
        }
    }
}

/// One rule of filters, with what search builds once per query.
struct Leaf {
    /// The filters, `collection` taken out (it is `member`).
    filters: Filters,
    need: Option<Need<'static>>,
    /// Faces added at this time or later pass (`added_within_days`).
    cutoff: Option<String>,
    /// Faces last activated at this time or later pass (`activated_within_days`).
    activated_cutoff: Option<String>,
    member: Option<Member>,
}

impl Leaf {
    fn passes(&self, row: &Row, done: &[bool]) -> bool {
        passes(&self.filters, &row.text, &row.facts)
            && failed_added(self.cutoff.as_deref(), row.added.as_deref()) == 0
            && failed_activated(
                self.activated_cutoff.as_deref(),
                row.last_activated.as_deref(),
            ) == 0
            // As search's `supports` filter: complete or a near miss passes,
            // unknown coverage fails.
            && self
                .need
                .as_ref()
                .is_none_or(|need| row.cmap.as_ref().is_some_and(|set| need.passes(set)))
            && self.member.as_ref().is_none_or(|m| m.holds(row, done))
    }
}

struct Compiler<'a> {
    conn: &'a Connection,
    rules: &'a Rules,
    known: &'a HashMap<CollectionId, usize>,
    /// The smart collections whose rules led here.
    path: Vec<CollectionId>,
}

impl Compiler<'_> {
    fn member(&mut self, id: CollectionId) -> FmResult<Member> {
        let Some((name, rule)) = self.rules.get(&id) else {
            return Ok(Member::Nothing);
        };
        let Some(rule) = rule else {
            return Ok(Member::Manual(id));
        };
        // Reached from another rule (never the collection asked for, whose
        // own rule is what is being worked out).
        if !self.path.is_empty()
            && let Some(at) = self.known.get(&id)
        {
            return Ok(Member::Known(*at));
        }
        if self.path.contains(&id) {
            return Err(FmError::invalid(format!(
                "The rules of \"{name}\" include the collection itself."
            )));
        }
        self.path.push(id);
        let group = self.group(rule)?;
        self.path.pop();
        Ok(Member::Smart(group))
    }

    fn group(&mut self, group: &RuleGroup) -> FmResult<Group> {
        let rules = group
            .rules
            .iter()
            .map(|rule| match rule {
                Rule::Filters { filters } => Ok(Compiled::Leaf(Box::new(self.leaf(filters)?))),
                Rule::Group { group } => Ok(Compiled::Group(self.group(group)?)),
            })
            .collect::<FmResult<_>>()?;
        Ok(Group {
            op: group.op,
            rules,
        })
    }

    fn leaf(&mut self, filters: &Filters) -> FmResult<Leaf> {
        let mut filters = filters.clone();
        let member = filters
            .collection
            .take()
            .map(|id| self.member(id))
            .transpose()?;
        Ok(Leaf {
            need: need(&filters)?,
            cutoff: cutoff(self.conn, filters.added_within_days)?,
            activated_cutoff: cutoff(self.conn, filters.activated_within_days)?,
            filters,
            member,
        })
    }
}

/// The characters a `supports` filter asks for; `None` when it asks for
/// none. An unknown language preset is `InvalidRequest`.
pub(in crate::index) fn need(filters: &Filters) -> FmResult<Option<Need<'static>>> {
    Ok(match &filters.supports {
        None => None,
        Some(Supports::Languages { codes }) => Some(Need::languages(codes)?),
        Some(Supports::Text { text }) => Some(Need::text(text)),
    }
    .filter(|need| !need.is_empty()))
}

/// The time `days` days ago, as `face.added` and `face.last_activated`
/// write it (RFC 3339, UTC).
pub(in crate::index) fn cutoff(conn: &Connection, days: Option<u32>) -> FmResult<Option<String>> {
    days.map(|days| {
        conn.query_row(
            "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now', ?1)",
            [format!("-{days} days")],
            |row| row.get(0),
        )
        .map_err(|e| db_error(&e))
    })
    .transpose()
}

impl Index {
    /// The faces in each smart collection now. A smart collection whose
    /// rule leads back to itself, or names an unknown language, holds none.
    pub(in crate::index) fn smart_face_counts(&self) -> FmResult<HashMap<CollectionId, u32>> {
        let rules = load_rules(&self.conn)?;
        if rules.values().all(|(_, rule)| rule.is_none()) {
            return Ok(HashMap::new());
        }
        self.refresh_search_cache()?;
        let cache = self.search_cache.borrow();
        // In id order: a rule names collections that existed before it, so
        // their results are usually known by then. A rule that failed to
        // compile is never known: a rule naming it compiles it again and
        // fails the same way, as a search by it does.
        let mut smart: Vec<CollectionId> = rules
            .iter()
            .filter(|(_, (_, rule))| rule.is_some())
            .map(|(id, _)| *id)
            .collect();
        smart.sort_unstable();
        let mut known: HashMap<CollectionId, usize> = HashMap::new();
        let mut members: Vec<Option<Member>> = Vec::with_capacity(smart.len());
        for (at, id) in smart.iter().enumerate() {
            match Member::compile_with(&self.conn, &rules, &known, *id) {
                Ok(member) => {
                    known.insert(*id, at);
                    members.push(Some(member));
                }
                Err(e) if e.code() == fm_api::ErrorCode::InvalidRequest => members.push(None),
                Err(e) => return Err(e),
            }
        }
        // Every rule runs on each row in turn, so a rule naming an earlier
        // collection reads that collection's result for the row; the rows
        // are shared out over the cores.
        let rows = cache.rows.as_slice();
        let chunks: Vec<std::ops::Range<usize>> = (0..rows.len())
            .step_by(COUNT_CHUNK)
            .map(|start| start..rows.len().min(start + COUNT_CHUNK))
            .collect();
        let counted = map_ordered(
            &chunks,
            |range| {
                let mut counts = vec![0_u32; members.len()];
                let mut done: Vec<bool> = Vec::with_capacity(members.len());
                for row in &rows[range.clone()] {
                    done.clear();
                    for (count, member) in counts.iter_mut().zip(&members) {
                        let inside = member.as_ref().is_some_and(|m| m.holds(row, &done));
                        *count += u32::from(inside);
                        done.push(inside);
                    }
                }
                counts
            },
            &mut |_| {},
        );
        let mut out: HashMap<CollectionId, u32> = smart.iter().map(|id| (*id, 0)).collect();
        for counts in counted {
            for (id, count) in smart.iter().zip(counts) {
                if let Some(total) = out.get_mut(id) {
                    *total = total.saturating_add(count);
                }
            }
        }
        Ok(out)
    }
}
