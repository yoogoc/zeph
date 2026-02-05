//! Autosurgeon extensions for GPUI types
//!
//! This module provides `Reconcile` and `Hydrate` implementations for GPUI's `SharedString`
//! type, allowing it to be used with autosurgeon CRDT documents.

use autosurgeon::{Hydrate, HydrateError, Prop, ReadDoc, Reconcile, Reconciler};
use gpui::SharedString;

/// Autosurgeon adapter module for `SharedString`.
///
/// Use with `#[autosurgeon(with = "shared_string_ext")]` attribute on struct fields.
///
/// # Example
/// ```ignore
/// use autosurgeon::{Hydrate, Reconcile};
/// use gpui::SharedString;
///
/// #[derive(Reconcile, Hydrate)]
/// struct MyStruct {
///     #[autosurgeon(with = "crate::autosurgeon_ext::shared_string")]
///     name: SharedString,
/// }
/// ```
pub mod shared_string {
    use super::*;

    /// Reconcile a `SharedString` to an automerge document as a string value.
    pub fn reconcile<R: Reconciler>(value: &SharedString, reconciler: R) -> Result<(), R::Error> {
        value.as_str().reconcile(reconciler)
    }

    /// Hydrate a `SharedString` from an automerge document.
    pub fn hydrate<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: Prop<'a>,
    ) -> Result<SharedString, HydrateError> {
        let s = String::hydrate(doc, obj, prop)?;
        Ok(SharedString::from(s))
    }
}

/// Autosurgeon adapter module for `Option<SharedString>`.
///
/// Use with `#[autosurgeon(with = "opt_shared_string_ext")]` attribute on struct fields.
///
/// # Example
/// ```ignore
/// use autosurgeon::{Hydrate, Reconcile};
/// use gpui::SharedString;
///
/// #[derive(Reconcile, Hydrate)]
/// struct MyStruct {
///     #[autosurgeon(with = "crate::autosurgeon_ext::opt_shared_string")]
///     description: Option<SharedString>,
/// }
/// ```
pub mod opt_shared_string {
    use super::*;

    /// Reconcile an `Option<SharedString>` to an automerge document.
    pub fn reconcile<R: Reconciler>(
        value: &Option<SharedString>,
        reconciler: R,
    ) -> Result<(), R::Error> {
        match value {
            Some(s) => s.as_str().reconcile(reconciler),
            None => Option::<String>::None.reconcile(reconciler),
        }
    }

    /// Hydrate an `Option<SharedString>` from an automerge document.
    pub fn hydrate<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: Prop<'a>,
    ) -> Result<Option<SharedString>, HydrateError> {
        match Option::<String>::hydrate(doc, obj, prop)? {
            Some(s) => Ok(Some(SharedString::from(s))),
            None => Ok(None),
        }
    }
}

/// Autosurgeon adapter module for `Vec<SharedString>`.
///
/// Use with `#[autosurgeon(with = "vec_shared_string_ext")]` attribute on struct fields.
pub mod vec_shared_string {
    use super::*;

    /// Reconcile a `Vec<SharedString>` to an automerge document as a list of strings.
    pub fn reconcile<R: Reconciler>(
        value: &Vec<SharedString>,
        reconciler: R,
    ) -> Result<(), R::Error> {
        let strings: Vec<String> = value.iter().map(|s| s.to_string()).collect();
        strings.reconcile(reconciler)
    }

    /// Hydrate a `Vec<SharedString>` from an automerge document.
    pub fn hydrate<'a, D: ReadDoc>(
        doc: &D,
        obj: &automerge::ObjId,
        prop: Prop<'a>,
    ) -> Result<Vec<SharedString>, HydrateError> {
        let strings = Vec::<String>::hydrate(doc, obj, prop)?;
        Ok(strings.into_iter().map(SharedString::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use automerge::AutoCommit;
    use autosurgeon::{hydrate, reconcile, Hydrate, Reconcile};

    #[derive(Debug, Clone, PartialEq, Reconcile, Hydrate)]
    struct TestStruct {
        #[autosurgeon(with = "super::shared_string")]
        name: SharedString,
        #[autosurgeon(with = "super::opt_shared_string")]
        description: Option<SharedString>,
        #[autosurgeon(with = "super::vec_shared_string")]
        tags: Vec<SharedString>,
    }

    #[test]
    fn test_shared_string_roundtrip() {
        let original = TestStruct {
            name: "test".into(),
            description: Some("a description".into()),
            tags: vec!["tag1".into(), "tag2".into()],
        };

        let mut doc = AutoCommit::new();
        reconcile(&mut doc, &original).unwrap();

        let hydrated: TestStruct = hydrate(&doc).unwrap();
        assert_eq!(original, hydrated);
    }

    #[test]
    fn test_shared_string_none_option() {
        let original = TestStruct {
            name: "test".into(),
            description: None,
            tags: vec![],
        };

        let mut doc = AutoCommit::new();
        reconcile(&mut doc, &original).unwrap();

        let hydrated: TestStruct = hydrate(&doc).unwrap();
        assert_eq!(original, hydrated);
    }
}
