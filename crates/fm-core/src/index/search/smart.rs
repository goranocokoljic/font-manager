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

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use fm_api::CollectionId;
use fm_api::organization::{Rule, RuleGroup, RuleOp};
use fm_api::search::{Filters, Supports};
use rusqlite::Connection;

use super::{Index, Row};
use crate::coverage::Need;
use crate::error::{FmError, FmResult};
use crate::filter::{failed_added, failed_supports, passes};
use crate::index::db_error;

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
    /// The faces (by id) a smart collection was already found to hold.
    Known(Rc<HashSet<u32>>),
}

impl Member {
    /// Compiles the test for the collection `id`. `conn` gives the time
    /// for `added_within_days`.
    pub(in crate::index) fn compile(
        conn: &Connection,
        rules: &Rules,
        id: CollectionId,
    ) -> FmResult<Self> {
        Self::compile_with(conn, rules, &HashMap::new(), id)
    }

    /// [`Member::compile`], taking the members of the smart collections in
    /// `known` as they are instead of running their rules again.
    fn compile_with(
        conn: &Connection,
        rules: &Rules,
        known: &HashMap<CollectionId, Rc<HashSet<u32>>>,
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

    pub(super) fn holds(&self, row: &Row) -> bool {
        match self {
            Self::Manual(id) => row.facts.collections.contains(id),
            Self::Smart(group) => group.passes(row),
            Self::Nothing => false,
            Self::Known(faces) => faces.contains(&row.id),
        }
    }
}

/// A compiled [`RuleGroup`].
pub(in crate::index) struct Group {
    op: RuleOp,
    rules: Vec<Compiled>,
}

impl Group {
    fn passes(&self, row: &Row) -> bool {
        match self.op {
            RuleOp::And => self.rules.iter().all(|r| r.passes(row)),
            // An empty group passes every face, whatever its operator.
            RuleOp::Or => self.rules.is_empty() || self.rules.iter().any(|r| r.passes(row)),
        }
    }
}

enum Compiled {
    Leaf(Box<Leaf>),
    Group(Group),
}

impl Compiled {
    fn passes(&self, row: &Row) -> bool {
        match self {
            Self::Leaf(leaf) => leaf.passes(row),
            Self::Group(group) => group.passes(row),
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
    member: Option<Member>,
}

impl Leaf {
    fn passes(&self, row: &Row) -> bool {
        passes(&self.filters, &row.text, &row.facts)
            && failed_added(self.cutoff.as_deref(), row.added.as_deref()) == 0
            && self.need.as_ref().is_none_or(|need| {
                let fit = row.cmap.as_ref().map(|set| need.check(set, &row.locl));
                failed_supports(fit.as_ref()) == 0
            })
            && self.member.as_ref().is_none_or(|m| m.holds(row))
    }
}

struct Compiler<'a> {
    conn: &'a Connection,
    rules: &'a Rules,
    known: &'a HashMap<CollectionId, Rc<HashSet<u32>>>,
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
            && let Some(faces) = self.known.get(&id)
        {
            return Ok(Member::Known(Rc::clone(faces)));
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

/// The time `days` days ago, as `face.added` writes it (RFC 3339, UTC).
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
        // their members are usually known by then.
        let mut smart: Vec<CollectionId> = rules
            .iter()
            .filter(|(_, (_, rule))| rule.is_some())
            .map(|(id, _)| *id)
            .collect();
        smart.sort_unstable();
        let mut known: HashMap<CollectionId, Rc<HashSet<u32>>> = HashMap::new();
        let mut out = HashMap::new();
        for id in smart {
            let faces: HashSet<u32> = match Member::compile_with(&self.conn, &rules, &known, id) {
                Ok(member) => cache
                    .rows
                    .iter()
                    .filter(|row| member.holds(row))
                    .map(|row| row.id)
                    .collect(),
                Err(e) if e.code() == fm_api::ErrorCode::InvalidRequest => HashSet::new(),
                Err(e) => return Err(e),
            };
            out.insert(id, u32::try_from(faces.len()).unwrap_or(u32::MAX));
            known.insert(id, Rc::new(faces));
        }
        Ok(out)
    }
}
