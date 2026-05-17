//! Combining selected pages from several PDFs into one document.

use std::collections::{BTreeMap, HashSet};

use lopdf::{Dictionary, Document, Object, ObjectId};
use thiserror::Error;

/// Attributes a `Page` may inherit from its `Pages` ancestors.
const INHERITABLE: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// Keys on a reused `/Pages` node that must not leak into the merged tree.
const STALE_PAGES_KEYS: [&[u8]; 5] = [b"MediaBox", b"CropBox", b"Resources", b"Rotate", b"Parent"];

/// Catalog entries that point into structures we drop while merging.
const DROPPED_CATALOG_KEYS: [&[u8]; 5] = [
    b"Outlines",
    b"OpenAction",
    b"Names",
    b"AcroForm",
    b"PageLabels",
];

/// Why a merge could not be produced.
#[derive(Debug, Error)]
pub enum MergeError {
    #[error("{path}: input has no usable document catalog / page tree")]
    NoCatalog { path: String },
    #[error("{path}: page {page} unexpectedly missing")]
    PageMissing { path: String, page: u32 },
    #[error("{path}: broken PDF object: {source}")]
    BrokenObject {
        path: String,
        #[source]
        source: lopdf::Error,
    },
}

/// Merge selected pages from several documents into one, preserving order.
pub fn merge(sources: Vec<(String, Document, Vec<u32>)>) -> Result<Document, MergeError> {
    let collected = collect(sources)?;
    let Skeleton {
        catalog_id,
        pages_id,
        catalog,
        pages_node,
    } = collected.skeleton;

    let (major, minor) = collected.version;
    let mut document = Document::with_version(format!("{major}.{minor}"));
    copy_supporting_objects(&mut document, &collected.objects);
    install_pages(&mut document, &collected.pages, pages_id);
    document.objects.insert(
        pages_id,
        Object::Dictionary(finalize_pages_node(pages_node, &collected.pages)),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(finalize_catalog(catalog, pages_id)),
    );

    document.trailer.set("Root", catalog_id);
    carry_over_info(&mut document, collected.info_id);
    document.max_id = collected.next_id;
    compact(&mut document);
    Ok(document)
}

/// The catalog/page-tree skeleton extracted from the first source document.
struct Skeleton {
    catalog_id: ObjectId,
    pages_id: ObjectId,
    catalog: Dictionary,
    pages_node: Dictionary,
}

/// Renumbered objects and the selected pages gathered from all source docs.
struct Collected {
    /// Selected pages in output order, as (object id, flattened dict). A page
    /// picked more than once gets a fresh id so the page tree stays valid.
    pages: Vec<(ObjectId, Dictionary)>,
    /// Every object from every source, with globally unique ids.
    objects: BTreeMap<ObjectId, Object>,
    /// Catalog and page-tree root taken from the first source's trailer.
    skeleton: Skeleton,
    /// First `/Info` dictionary id seen, carried over to the merged trailer.
    info_id: Option<ObjectId>,
    /// Highest object id used so far, plus one.
    next_id: u32,
    /// Highest `(major, minor)` PDF version among the sources (floor `1.5`).
    version: (u32, u32),
}

fn collect(sources: Vec<(String, Document, Vec<u32>)>) -> Result<Collected, MergeError> {
    let mut iter = sources.into_iter();
    let (first_path, mut first_doc, first_selected) = iter
        .next()
        .expect("merge called with no sources; runner enforces at least one input");

    let mut next_id: u32 = 1;
    let mut version = parse_version(&first_doc.version);
    first_doc.renumber_objects_with(next_id);
    next_id = first_doc.max_id + 1;

    let skeleton = extract_skeleton(&first_doc, &first_path)?;
    let info_id = first_info_id(&first_doc);

    let mut pages: Vec<(ObjectId, Dictionary)> = Vec::new();
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();

    collect_one(
        &first_path,
        &first_doc,
        &first_selected,
        &mut pages,
        &mut next_id,
    )?;
    objects.extend(first_doc.objects);

    for (path, mut doc, selected) in iter {
        version = version.max(parse_version(&doc.version));
        doc.renumber_objects_with(next_id);
        next_id = doc.max_id + 1;
        collect_one(&path, &doc, &selected, &mut pages, &mut next_id)?;
        objects.extend(doc.objects);
    }

    Ok(Collected {
        pages,
        objects,
        skeleton,
        info_id,
        next_id,
        version,
    })
}

fn collect_one(
    path: &str,
    doc: &Document,
    selected: &[u32],
    pages: &mut Vec<(ObjectId, Dictionary)>,
    next_id: &mut u32,
) -> Result<(), MergeError> {
    let doc_pages = doc.get_pages();
    let mut seen = HashSet::new();
    for &page_no in selected {
        let src_id = *doc_pages
            .get(&page_no)
            .ok_or_else(|| MergeError::PageMissing {
                path: path.to_string(),
                page: page_no,
            })?;
        let dict = flatten_inherited(doc, src_id).map_err(|source| MergeError::BrokenObject {
            path: path.to_string(),
            source,
        })?;
        let id = if seen.insert(src_id) {
            src_id
        } else {
            let fresh = (*next_id, 0);
            *next_id += 1;
            fresh
        };
        pages.push((id, dict));
    }
    Ok(())
}

fn extract_skeleton(doc: &Document, path: &str) -> Result<Skeleton, MergeError> {
    let no_catalog = || MergeError::NoCatalog {
        path: path.to_string(),
    };
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|_| no_catalog())?;
    let catalog_dict = doc
        .get_object(catalog_id)
        .and_then(Object::as_dict)
        .map_err(|_| no_catalog())?
        .clone();
    let pages_id = catalog_dict
        .get(b"Pages")
        .and_then(Object::as_reference)
        .map_err(|_| no_catalog())?;
    let pages_node = doc
        .get_object(pages_id)
        .and_then(Object::as_dict)
        .map_err(|_| no_catalog())?
        .clone();
    Ok(Skeleton {
        catalog_id,
        pages_id,
        catalog: catalog_dict,
        pages_node,
    })
}

fn first_info_id(doc: &Document) -> Option<ObjectId> {
    if let Ok(Object::Reference(id)) = doc.trailer.get(b"Info") {
        Some(*id)
    } else {
        None
    }
}

/// Parse a PDF header version like `"1.7"` into `(major, minor)`; missing or
/// non-numeric parts fall back to `major = 1`, `minor = 0`.
fn parse_version(version: &str) -> (u32, u32) {
    let mut parts = version.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor)
}

/// Copy inheritable attributes from the page-tree ancestors onto the page dict
/// itself, so pages keep their geometry once detached from their original tree.
fn flatten_inherited(doc: &Document, page_id: ObjectId) -> Result<Dictionary, lopdf::Error> {
    let mut dict = doc.get_object(page_id)?.as_dict()?.clone();

    let mut cursor = dict.clone();
    let mut visited = HashSet::from([page_id]);
    while let Some(parent_id) = cursor
        .get(b"Parent")
        .ok()
        .and_then(|o| o.as_reference().ok())
    {
        if !visited.insert(parent_id) {
            break; // circular page tree
        }
        let Ok(parent) = doc.get_object(parent_id).and_then(Object::as_dict).cloned() else {
            break;
        };
        for key in INHERITABLE {
            if dict.get(key).is_err()
                && let Ok(value) = parent.get(key)
            {
                dict.set(key.to_vec(), value.clone());
            }
        }
        cursor = parent;
    }
    dict.remove(b"Parent");
    Ok(dict)
}

/// Copy every object that is not part of the page tree we are rebuilding: drop
/// page leaves (replaced by the flattened versions) and `/Catalog` / `/Pages`
/// nodes (the chosen ones are re-inserted afterwards; the rest are dead weight).
fn copy_supporting_objects(document: &mut Document, objects: &BTreeMap<ObjectId, Object>) {
    for (id, object) in objects {
        match object.type_name().unwrap_or_default() {
            b"Catalog" | b"Pages" | b"Page" => {}
            _ => {
                document.objects.insert(*id, object.clone());
            }
        }
    }
}

/// Insert each selected page, reparented to the single shared `/Pages` node.
fn install_pages(document: &mut Document, pages: &[(ObjectId, Dictionary)], parent: ObjectId) {
    for (id, dict) in pages {
        let mut dict = dict.clone();
        dict.set("Parent", parent);
        document.objects.insert(*id, Object::Dictionary(dict));
    }
}

/// Strip stale inherited attributes from the reused `/Pages` node and point it
/// at the selected pages.
fn finalize_pages_node(mut node: Dictionary, pages: &[(ObjectId, Dictionary)]) -> Dictionary {
    for key in STALE_PAGES_KEYS {
        node.remove(key);
    }
    node.set("Count", pages.len() as u32);
    node.set(
        "Kids",
        pages
            .iter()
            .map(|(id, _)| Object::Reference(*id))
            .collect::<Vec<_>>(),
    );
    node
}

/// Point the reused `/Catalog` at the new page tree and drop entries that
/// reference structures we no longer keep.
fn finalize_catalog(mut catalog: Dictionary, pages_id: ObjectId) -> Dictionary {
    catalog.set("Pages", pages_id);
    for key in DROPPED_CATALOG_KEYS {
        catalog.remove(key);
    }
    catalog
}

/// Reference the carried-over `/Info` dictionary from the trailer, if it survived.
fn carry_over_info(document: &mut Document, info_id: Option<ObjectId>) {
    if let Some(id) = info_id
        && document.objects.contains_key(&id)
    {
        document.trailer.set("Info", id);
    }
}

/// Drop unreachable objects, compact ids, and compress streams.
fn compact(document: &mut Document) {
    document.prune_objects();
    document.renumber_objects();
    document.compress();
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
