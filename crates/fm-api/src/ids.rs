//! Opaque identifiers. Each is a `u32` on the wire (a plain `number` in
//! TypeScript); the core maps them to database row ids.

use serde::{Deserialize, Serialize};

macro_rules! id {
    ($($(#[$meta:meta])* $name:ident),* $(,)?) => {$(
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
            Serialize, Deserialize, specta::Type,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    )*};
}

id!(
    /// A watched folder, the system fonts, or the library folder.
    SourceId,
    /// A family: faces grouped by typographic family name.
    FamilyId,
    /// One face (style) inside a font file; a collection file has several.
    FaceId,
    /// A manual or smart collection.
    CollectionId,
    /// A free-form tag.
    TagId,
    /// A project set: fonts activated together.
    SetId,
    /// A long-running job (a scan); its progress arrives as events.
    JobId,
    /// A "Keep this one" action of the Problems view, for undo.
    ResolutionId,
);
