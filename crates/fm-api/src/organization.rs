//! Organization service: collections, tags, user metadata and project sets.

use serde::{Deserialize, Serialize};

use crate::common::AxisValue;
use crate::search::Filters;
use crate::{ApiResult, CollectionId, FaceId, FamilyId, SetId, TagId, VirtualStyleId};

/// A manual collection, or a smart one when `rules` is set. Siblings are
/// ordered by the user; `list_collections` returns them in that order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    /// Id.
    pub id: CollectionId,
    /// Parent, for nesting; absent at the top level.
    pub parent: Option<CollectionId>,
    /// Name.
    pub name: String,
    /// Smart collection rules; absent for a manual collection.
    pub rules: Option<Filters>,
    /// Faces in it now.
    pub face_count: u32,
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
}
