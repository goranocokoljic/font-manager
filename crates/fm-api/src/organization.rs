//! Organization service: collections, tags, user metadata and project sets.

use serde::{Deserialize, Serialize};

use crate::common::AxisValue;
use crate::library::{FaceSummary, LicenseType};
use crate::search::Filters;
use crate::{ApiResult, CollectionId, FaceId, FamilyId, SetId, TagId, VirtualStyleId};

/// A manual collection, or a smart one when `rules` is set. Siblings are
/// ordered by the user; `list_collections` returns them in that order.
///
/// A smart collection's members are worked out from the index each time
/// they are asked for, never stored: a scan, a tag, a rating or an
/// activation shows in the next `search` and `list_collections`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    /// Id.
    pub id: CollectionId,
    /// Parent, for nesting; absent at the top level.
    pub parent: Option<CollectionId>,
    /// Name.
    pub name: String,
    /// Smart collection rules; absent for a manual collection. Set for
    /// every smart collection: the filters of a rule that is one AND group
    /// of one filter set (as phase 1 stored them), else empty filters. The
    /// full rule is `rule_group`.
    pub rules: Option<Filters>,
    /// The full rule of a smart collection; absent for a manual one. A rule
    /// stored as plain filters reads as one AND group of them.
    #[serde(default)]
    pub rule_group: Option<RuleGroup>,
    /// Faces in it now.
    pub face_count: u32,
}

/// How a [`RuleGroup`] combines its rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RuleOp {
    /// A face must pass every rule.
    And,
    /// A face must pass at least one rule.
    Or,
}

/// A smart collection's rule: a group of rules combined with AND or OR. A
/// group with no rules passes every face.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleGroup {
    /// AND or OR.
    pub op: RuleOp,
    /// The rules.
    #[serde(default)]
    pub rules: Vec<Rule>,
}

/// One rule of a [`RuleGroup`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Rule {
    /// A face passes when it passes these filters, as search applies them.
    /// `collection` may name another smart collection: its rule applies
    /// (a rule that reaches its own collection is `InvalidRequest`).
    #[serde(rename_all = "camelCase")]
    Filters {
        /// The filters.
        filters: Filters,
    },
    /// A nested group.
    #[serde(rename_all = "camelCase")]
    Group {
        /// The group.
        group: RuleGroup,
    },
}

impl RuleGroup {
    /// The filters of a group that is one AND rule of plain filters (a rule
    /// as phase 1 stored it); `None` for any other group.
    #[must_use]
    pub fn plain_filters(&self) -> Option<&Filters> {
        match (self.op, self.rules.as_slice()) {
            (RuleOp::And, [Rule::Filters { filters }]) => Some(filters),
            _ => None,
        }
    }

    /// Every collection its filters name, nested groups included.
    #[must_use]
    pub fn collections(&self) -> Vec<CollectionId> {
        let mut out = Vec::new();
        for rule in &self.rules {
            match rule {
                Rule::Filters { filters } => out.extend(filters.collection),
                Rule::Group { group } => out.extend(group.collections()),
            }
        }
        out
    }
}

impl From<Filters> for RuleGroup {
    /// Plain filters (a phase-1 rule) as one AND group of them.
    fn from(filters: Filters) -> Self {
        Self {
            op: RuleOp::And,
            rules: vec![Rule::Filters { filters }],
        }
    }
}

/// Create a collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionRequest {
    /// Name; unique among its siblings.
    pub name: String,
    /// Parent.
    #[serde(default)]
    pub parent: Option<CollectionId>,
    /// Rules, to make it smart.
    #[serde(default)]
    pub rules: Option<Filters>,
    /// A full rule, to make it smart with AND/OR groups. Setting both this
    /// and `rules` is `InvalidRequest`.
    #[serde(default)]
    pub rule_group: Option<RuleGroup>,
}

/// Rename a collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RenameCollectionRequest {
    /// The collection.
    pub collection: CollectionId,
    /// New name.
    pub name: String,
}

/// Move a collection under another parent, or to another place among its
/// siblings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MoveCollectionRequest {
    /// The collection.
    pub collection: CollectionId,
    /// The new parent; absent for the top level. The collection itself or
    /// one of its children is `InvalidRequest`.
    #[serde(default)]
    pub parent: Option<CollectionId>,
    /// Its place among the new siblings, 0 first; past the end means last.
    pub position: u32,
}

/// Replace the rule of a smart collection (E14.1). The rule is checked as
/// `create_collection` checks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCollectionRulesRequest {
    /// The smart collection; a manual one is `InvalidRequest`.
    pub collection: CollectionId,
    /// The new rule.
    pub rule_group: RuleGroup,
}

/// Add faces to, or remove them from, a manual collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionFacesRequest {
    /// The collection; a smart one is `InvalidRequest`.
    pub collection: CollectionId,
    /// The faces.
    pub faces: Vec<FaceId>,
    /// Whole families: every face of each, as the library lists it.
    #[serde(default)]
    pub families: Vec<FamilyId>,
}

/// A tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    /// Id.
    pub id: TagId,
    /// Name.
    pub name: String,
    /// Faces carrying it.
    pub face_count: u32,
}

/// Attach tags to faces; tags that do not exist yet are created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TagFacesRequest {
    /// The faces.
    pub faces: Vec<FaceId>,
    /// Tag names.
    pub tags: Vec<String>,
    /// Whole families (E13.4): every face of each, as the library lists it.
    #[serde(default)]
    pub families: Vec<FamilyId>,
}

/// The fonts a selection or a drag names (E13.4): faces and whole families.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionTagsRequest {
    /// The faces.
    pub faces: Vec<FaceId>,
    /// Whole families: every face of each, as the library lists it.
    #[serde(default)]
    pub families: Vec<FamilyId>,
}

/// The tags the faces of a selection carry (E13.4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionTags {
    /// Every face the selection names, each once: the faces first, then the
    /// faces of each family.
    pub faces: Vec<FaceId>,
    /// Every tag at least one of those faces carries, A to Z.
    pub tags: Vec<SelectionTag>,
}

/// One tag of a [`SelectionTags`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionTag {
    /// The tag.
    pub tag: TagId,
    /// How many of the selection's faces carry it: all of them, or some
    /// (a mixed state).
    pub face_count: u32,
    /// True when every face of the selection carries it.
    pub all: bool,
}

/// Detach tags from faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UntagFacesRequest {
    /// The faces.
    pub faces: Vec<FaceId>,
    /// The tags.
    pub tags: Vec<TagId>,
}

/// Change user metadata on faces; absent fields are left as they are.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UserMetaUpdate {
    /// The faces.
    pub faces: Vec<FaceId>,
    /// Favorite or not.
    #[serde(default)]
    pub favorite: Option<bool>,
    /// 1–5 stars; 0 clears the rating.
    #[serde(default)]
    pub rating: Option<u8>,
    /// Notes; an empty string clears them.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Set the notes of a family. They belong to the family, not to one of its
/// faces: they stay when its faces are indexed again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FamilyNotesUpdate {
    /// The family.
    pub family: FamilyId,
    /// Notes; an empty string clears them.
    pub notes: String,
}

/// Declare the license of a family (E15.2). It applies to every face of the
/// family and replaces what was declared before: a field left absent is
/// cleared. The receipt is kept unless the request clears or replaces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LicenseDeclaration {
    /// The family.
    pub family: FamilyId,
    /// The license type; absent clears it.
    #[serde(default)]
    pub license_type: Option<LicenseType>,
    /// How many seats the license covers; absent clears it, below 0 is
    /// `InvalidRequest`.
    #[serde(default)]
    pub seats: Option<i32>,
    /// The purchase date as `YYYY-MM-DD`; absent or empty clears it, any
    /// other form is `InvalidRequest`.
    #[serde(default)]
    pub purchase_date: Option<String>,
    /// What happens to the attached receipt.
    #[serde(default)]
    pub receipt: ReceiptChange,
}

/// What [`LicenseDeclaration`] does to a family's receipt.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ReceiptChange {
    /// Leave the receipt as it is.
    #[default]
    Keep,
    /// Remove the receipt: the copy the app made goes to the OS trash.
    Clear,
    /// Attach a copy of the file at `path`; the file itself is never moved.
    /// A receipt attached before goes to the OS trash. A file that is not
    /// there is `InvalidRequest`.
    #[serde(rename_all = "camelCase")]
    Attach {
        /// The receipt file the user picked.
        path: String,
        /// Where the copy goes.
        place: ReceiptPlace,
    },
}

/// Where the app keeps the copy of a receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ReceiptPlace {
    /// Next to the font, in its source folder's `.fontlibrary/` folder, so a
    /// synced folder carries it to teammates. A system font folder is
    /// `InvalidRequest`.
    NextToFont,
    /// In the app's data folder, on this machine only.
    AppData,
}

/// A project set: fonts activated together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSet {
    /// Id.
    pub id: SetId,
    /// Name.
    pub name: String,
    /// The faces.
    pub faces: Vec<FaceId>,
}

/// Create a project set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateSetRequest {
    /// Name.
    pub name: String,
    /// The faces.
    pub faces: Vec<FaceId>,
}

/// One font of an exported set, referenced by content so a teammate's copy
/// at another path still matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetEntry {
    /// Family name.
    pub family: String,
    /// Style name.
    pub style: String,
    /// blake3 hash of the file, hex.
    pub hash: String,
}

/// A project set as portable JSON (no files).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetExport {
    /// Set name.
    pub name: String,
    /// Its fonts.
    pub fonts: Vec<SetEntry>,
}

/// The result of importing a set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSetResponse {
    /// The new set, holding the fonts found in this library.
    pub set: ProjectSet,
    /// Fonts not found in this library.
    pub missing: Vec<SetEntry>,
}

/// Pin an instance of a variable face as a virtual style of a manual
/// collection or a project set: exactly one of `collection` and `set`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PinInstanceRequest {
    /// The variable face.
    pub face: FaceId,
    /// Style name ("Condensed Semibold"); unique per face in its collection
    /// or set.
    pub name: String,
    /// Axis coordinates; an axis left out is at its default.
    #[serde(default)]
    pub coordinates: Vec<AxisValue>,
    /// The manual collection that holds it.
    #[serde(default)]
    pub collection: Option<CollectionId>,
    /// The project set that holds it.
    #[serde(default)]
    pub set: Option<SetId>,
}

/// An instance of a variable face, pinned as a style of its own. The font
/// file is not changed: activating it activates the face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VirtualStyle {
    /// Id.
    pub id: VirtualStyleId,
    /// The variable face it is an instance of.
    pub face: FaceId,
    /// Style name.
    pub name: String,
    /// A coordinate for every axis of the face, in the font's axis order.
    pub coordinates: Vec<AxisValue>,
    /// The manual collection that holds it, if a collection does.
    #[serde(default)]
    pub collection: Option<CollectionId>,
    /// The project set that holds it, if a set does.
    #[serde(default)]
    pub set: Option<SetId>,
}

/// The virtual styles of one manual collection or one project set: exactly
/// one of `collection` and `set`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VirtualStylesRequest {
    /// The collection.
    #[serde(default)]
    pub collection: Option<CollectionId>,
    /// The set.
    #[serde(default)]
    pub set: Option<SetId>,
}

/// The Quick Collection (E13.3): one list of faces the user gathers while
/// browsing, to review and then keep as a collection or a tag. It is kept in
/// the index, so it outlives a restart.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickCollection {
    /// Its faces in the order they were added, first added first. A face
    /// the library does not show (its file is gone, its source paused) is
    /// left out while it is hidden.
    pub faces: Vec<FaceSummary>,
}

/// Add faces to, or remove them from, the Quick Collection.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuickCollectionRequest {
    /// The faces. Adding a face already in it keeps its place; removing a
    /// face not in it does nothing.
    pub faces: Vec<FaceId>,
}

/// Collections, tags, user metadata and project sets.
pub trait OrganizationService {
    /// Every collection, parents before children.
    fn list_collections(&self) -> ApiResult<Vec<Collection>>;
    /// Create a collection.
    fn create_collection(&self, req: CreateCollectionRequest) -> ApiResult<Collection>;
    /// Rename a collection.
    fn rename_collection(&self, req: RenameCollectionRequest) -> ApiResult<Collection>;
    /// Nest a collection under another one, or reorder it among its siblings.
    fn move_collection(&self, req: MoveCollectionRequest) -> ApiResult<Collection>;
    /// Replace the rule of a smart collection (E14.1). A manual collection,
    /// a group with no rules or a weight or width range whose minimum is
    /// above its maximum is `InvalidRequest`, as is a rule that leads back
    /// to the collection; nothing is saved then.
    fn update_collection_rules(&self, req: UpdateCollectionRulesRequest) -> ApiResult<Collection>;
    /// Delete a collection and its children. Faces are not touched.
    fn delete_collection(&self, collection: CollectionId) -> ApiResult<()>;
    /// Add faces or families to a manual collection; a face can be in many.
    fn add_to_collection(&self, req: CollectionFacesRequest) -> ApiResult<Collection>;
    /// Remove faces from a manual collection.
    fn remove_from_collection(&self, req: CollectionFacesRequest) -> ApiResult<Collection>;
    /// Every tag.
    fn list_tags(&self) -> ApiResult<Vec<Tag>>;
    /// Attach tags; returns the tags touched.
    fn tag_faces(&self, req: TagFacesRequest) -> ApiResult<Vec<Tag>>;
    /// Detach tags.
    fn untag_faces(&self, req: UntagFacesRequest) -> ApiResult<()>;
    /// The tags the faces of a selection carry, with how many carry each
    /// (E13.4). An unknown face or family is `NotFound`.
    fn selection_tags(&self, req: SelectionTagsRequest) -> ApiResult<SelectionTags>;
    /// Favorite, rating, notes.
    fn update_user_meta(&self, req: UserMetaUpdate) -> ApiResult<()>;
    /// Every project set.
    fn list_sets(&self) -> ApiResult<Vec<ProjectSet>>;
    /// Create a project set.
    fn create_set(&self, req: CreateSetRequest) -> ApiResult<ProjectSet>;
    /// Delete a project set. Faces are not touched.
    fn delete_set(&self, set: SetId) -> ApiResult<()>;
    /// A set as portable JSON.
    fn export_set(&self, set: SetId) -> ApiResult<SetExport>;
    /// Create a set from portable JSON, matching fonts by hash.
    fn import_set(&self, export: SetExport) -> ApiResult<ImportSetResponse>;
    /// Pin an instance of a variable face as a virtual style of a manual
    /// collection or a set. A static face, an unknown axis or a coordinate
    /// outside its axis is `InvalidRequest`.
    fn pin_instance(&self, req: PinInstanceRequest) -> ApiResult<VirtualStyle>;
    /// The virtual styles of a collection or a set, A to Z.
    fn list_virtual_styles(&self, req: VirtualStylesRequest) -> ApiResult<Vec<VirtualStyle>>;
    /// Delete a virtual style. The face is not touched.
    fn delete_virtual_style(&self, style: VirtualStyleId) -> ApiResult<()>;
    /// Set the notes of a family (E13.5); an empty string clears them.
    fn update_family_notes(&self, req: FamilyNotesUpdate) -> ApiResult<()>;
    /// Declare the license of a family (E15.2): type, seats, purchase date
    /// and receipt. An unknown family is `NotFound`; a bad date, seats below
    /// 0 or a receipt file that is not there is `InvalidRequest`.
    fn declare_license(&self, req: LicenseDeclaration) -> ApiResult<()>;
    /// The Quick Collection (E13.3), first added first.
    fn quick_collection(&self) -> ApiResult<QuickCollection>;
    /// Add faces to the end of the Quick Collection; an unknown face is
    /// `NotFound` and adds nothing. Returns the Quick Collection.
    fn add_to_quick_collection(&self, req: QuickCollectionRequest) -> ApiResult<QuickCollection>;
    /// Remove faces from the Quick Collection. Returns what is left.
    fn remove_from_quick_collection(
        &self,
        req: QuickCollectionRequest,
    ) -> ApiResult<QuickCollection>;
    /// Empty the Quick Collection. The faces are not touched.
    fn clear_quick_collection(&self) -> ApiResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::FontFormat;

    /// A collection as phase 1 stored and sent it: plain filters, no group.
    const PHASE_1: &str = r#"{"id":4,"parent":null,"name":"Variable TTF",
        "rules":{"variable":true,"formats":["ttf"]},"faceCount":0}"#;

    #[test]
    fn a_phase_1_rule_reads_as_one_and_group() {
        let c: Collection = serde_json::from_str(PHASE_1).expect("phase-1 collection");
        assert_eq!(c.rule_group, None);
        let rules = c.rules.expect("rules");
        let group = RuleGroup::from(rules.clone());
        assert_eq!(group.op, RuleOp::And);
        assert_eq!(group.rules, vec![Rule::Filters { filters: rules }]);
        let req: CreateCollectionRequest =
            serde_json::from_str(r#"{"name":"Old","rules":{"variable":true}}"#).expect("request");
        assert_eq!(req.rule_group, None);
        assert_eq!(req.rules.and_then(|r| r.variable), Some(true));
    }

    #[test]
    fn a_rule_group_round_trips_with_nested_groups() {
        let group = RuleGroup {
            op: RuleOp::Or,
            rules: vec![
                Rule::Filters {
                    filters: Filters {
                        added_within_days: Some(7),
                        ..Filters::default()
                    },
                },
                Rule::Group {
                    group: RuleGroup {
                        op: RuleOp::And,
                        rules: vec![Rule::Filters {
                            filters: Filters {
                                formats: vec![FontFormat::Otf],
                                collection: Some(CollectionId(3)),
                                ..Filters::default()
                            },
                        }],
                    },
                },
            ],
        };
        let json = serde_json::to_value(&group).expect("json");
        assert_eq!(json["op"], "or");
        assert_eq!(json["rules"][0]["kind"], "filters");
        assert_eq!(json["rules"][0]["filters"]["addedWithinDays"], 7);
        assert_eq!(json["rules"][1]["kind"], "group");
        assert_eq!(json["rules"][1]["group"]["op"], "and");
        let back: RuleGroup = serde_json::from_value(json).expect("back");
        assert_eq!(back, group);
        let empty: RuleGroup = serde_json::from_str(r#"{"op":"and"}"#).expect("empty");
        assert!(empty.rules.is_empty());
    }
}
