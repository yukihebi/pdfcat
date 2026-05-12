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
    #[error("no pages selected")]
    NoPages,
    #[error("input has no usable document catalog / page tree")]
    NoCatalog,
    #[error("page {0} unexpectedly missing")]
    PageMissing(u32),
    #[error("broken PDF object: {0}")]
    BrokenObject(#[from] lopdf::Error),
}

/// Merge selected pages from several documents into one, preserving order.
pub fn merge(sources: Vec<(Document, Vec<u32>)>) -> Result<Document, MergeError> {
    let collected = collect(sources)?;
    if collected.pages.is_empty() {
        return Err(MergeError::NoPages);
    }
    let (catalog_id, pages_id) = collected.skeleton.ok_or(MergeError::NoCatalog)?;
    let catalog = clone_dict(&collected.objects, catalog_id)?;
    let pages_node = clone_dict(&collected.objects, pages_id)?;

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

/// Renumbered objects and the selected pages gathered from all source docs.
struct Collected {
    /// Selected pages in output order, as (object id, flattened dict). A page
    /// picked more than once gets a fresh id so the page tree stays valid.
    pages: Vec<(ObjectId, Dictionary)>,
    /// Every object from every source, with globally unique ids.
    objects: BTreeMap<ObjectId, Object>,
    /// `(catalog id, page-tree root id)` taken from the first source's trailer.
    skeleton: Option<(ObjectId, ObjectId)>,
    /// First `/Info` dictionary id seen, carried over to the merged trailer.
    info_id: Option<ObjectId>,
    /// Highest object id used so far, plus one.
    next_id: u32,
    /// Highest `(major, minor)` PDF version among the sources (floor `1.5`).
    version: (u32, u32),
}

fn collect(sources: Vec<(Document, Vec<u32>)>) -> Result<Collected, MergeError> {
    let mut collected = Collected {
        pages: Vec::new(),
        objects: BTreeMap::new(),
        skeleton: None,
        info_id: None,
        next_id: 1,
        version: (1, 5),
    };

    for (mut doc, selected) in sources {
        collected.version = collected.version.max(parse_version(&doc.version));
        doc.renumber_objects_with(collected.next_id);
        collected.next_id = doc.max_id + 1;
        if collected.skeleton.is_none() {
            let catalog_id = doc.trailer.get(b"Root").and_then(Object::as_reference)?;
            let pages_id = doc
                .get_object(catalog_id)
                .and_then(Object::as_dict)?
                .get(b"Pages")
                .and_then(Object::as_reference)?;
            collected.skeleton = Some((catalog_id, pages_id));
        }
        if collected.info_id.is_none()
            && let Ok(Object::Reference(id)) = doc.trailer.get(b"Info")
        {
            collected.info_id = Some(*id);
        }

        let pages = doc.get_pages(); // page number -> object id
        let mut seen = HashSet::new();
        for page_no in selected {
            let src_id = *pages
                .get(&page_no)
                .ok_or(MergeError::PageMissing(page_no))?;
            let dict = flatten_inherited(&doc, src_id)?;
            let id = if seen.insert(src_id) {
                src_id
            } else {
                let fresh = (collected.next_id, 0);
                collected.next_id += 1;
                fresh
            };
            collected.pages.push((id, dict));
        }
        collected.objects.extend(doc.objects);
    }
    Ok(collected)
}

/// Parse a PDF header version like `"1.7"` into `(major, minor)`; missing or
/// non-numeric parts fall back to `major = 1`, `minor = 0`.
fn parse_version(version: &str) -> (u32, u32) {
    let mut parts = version.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor)
}

/// Clone the dictionary stored at `id` (used for the reused catalog / pages node).
fn clone_dict(
    objects: &BTreeMap<ObjectId, Object>,
    id: ObjectId,
) -> Result<Dictionary, MergeError> {
    objects
        .get(&id)
        .and_then(|o| o.as_dict().ok())
        .cloned()
        .ok_or(MergeError::NoCatalog)
}

/// Copy inheritable attributes from the page-tree ancestors onto the page dict
/// itself, so pages keep their geometry once detached from their original tree.
fn flatten_inherited(doc: &Document, page_id: ObjectId) -> Result<Dictionary, MergeError> {
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
mod tests {
    use super::*;

    /// Build an `n`-page document whose pages carry a distinct `/MediaBox`
    /// width — a stand-in for page identity that survives the merge.
    fn doc_with_widths(widths: &[i64]) -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.add_object(Dictionary::new());
        let kids: Vec<Object> = widths
            .iter()
            .map(|&w| {
                let mut page = Dictionary::new();
                page.set("Type", "Page");
                page.set("Parent", pages_id);
                page.set("MediaBox", media_box(w));
                Object::Reference(doc.add_object(page))
            })
            .collect();
        let mut pages = Dictionary::new();
        pages.set("Type", "Pages");
        pages.set("Count", widths.len() as i64);
        pages.set("Kids", kids);
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let mut catalog = Dictionary::new();
        catalog.set("Type", "Catalog");
        catalog.set("Pages", pages_id);
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", catalog_id);
        doc
    }

    /// Like [`doc_with_widths`] but the `/MediaBox` lives on the `/Pages` node,
    /// so each page inherits it rather than declaring its own.
    fn doc_with_inherited_box(n: usize, width: i64) -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.add_object(Dictionary::new());
        let kids: Vec<Object> = (0..n)
            .map(|_| {
                let mut page = Dictionary::new();
                page.set("Type", "Page");
                page.set("Parent", pages_id);
                Object::Reference(doc.add_object(page))
            })
            .collect();
        let mut pages = Dictionary::new();
        pages.set("Type", "Pages");
        pages.set("Count", n as i64);
        pages.set("Kids", kids);
        pages.set("MediaBox", media_box(width));
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let mut catalog = Dictionary::new();
        catalog.set("Type", "Catalog");
        catalog.set("Pages", pages_id);
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", catalog_id);
        doc
    }

    fn media_box(width: i64) -> Object {
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(width),
            Object::Integer(100),
        ])
    }

    /// The `/MediaBox` width of every page, in page order.
    fn page_widths(doc: &Document) -> Vec<i64> {
        doc.get_pages()
            .values()
            .map(|&id| {
                let dict = doc.get_object(id).unwrap().as_dict().unwrap();
                let mb = dict.get(b"MediaBox").unwrap().as_array().unwrap();
                mb[2].as_i64().unwrap()
            })
            .collect()
    }

    fn page_ids(doc: &Document) -> Vec<ObjectId> {
        doc.get_pages().values().copied().collect()
    }

    #[test]
    fn concatenates_pages_in_order() {
        let merged = merge(vec![
            (doc_with_widths(&[10, 20, 30]), vec![1, 2, 3]),
            (doc_with_widths(&[40, 50]), vec![1, 2]),
        ])
        .unwrap();
        assert_eq!(page_widths(&merged), [10, 20, 30, 40, 50]);
        // The trailer points at a real Catalog whose /Pages has Count == 5.
        let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
        let pages_id = catalog.get(b"Pages").unwrap().as_reference().unwrap();
        let pages = merged.get_object(pages_id).unwrap().as_dict().unwrap();
        assert_eq!(pages.get(b"Count").unwrap().as_i64().unwrap(), 5);
    }

    #[test]
    fn selects_and_reorders_pages() {
        let merged = merge(vec![(
            doc_with_widths(&[10, 20, 30, 40, 50]),
            vec![5, 1, 3],
        )])
        .unwrap();
        assert_eq!(page_widths(&merged), [50, 10, 30]);
    }

    #[test]
    fn duplicate_page_gets_a_fresh_id() {
        let merged = merge(vec![(doc_with_widths(&[10, 20]), vec![1, 1, 2])]).unwrap();
        assert_eq!(page_widths(&merged), [10, 10, 20]);
        let ids = page_ids(&merged);
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 3, "every page must be a distinct object");
    }

    #[test]
    fn duplicate_then_more_inputs_keeps_ids_disjoint() {
        // A duplicated page in the first input must not steal an id that the
        // second input's objects will be renumbered onto.
        let merged = merge(vec![
            (doc_with_widths(&[10, 20]), vec![1, 1]),
            (doc_with_widths(&[30, 40]), vec![2, 1]),
        ])
        .unwrap();
        assert_eq!(page_widths(&merged), [10, 10, 40, 30]);
        let unique: HashSet<_> = page_ids(&merged).into_iter().collect();
        assert_eq!(unique.len(), 4);
    }

    #[test]
    fn keeps_supporting_objects_but_drops_outlines() {
        let mut doc = doc_with_widths(&[10]);
        // Attach a content stream to the page (a supporting object that must
        // survive) and an /Outlines entry to the catalog (which must not).
        let contents_id = doc.add_object(lopdf::Stream::new(Dictionary::new(), b"BT ET".to_vec()));
        let outlines_id = doc.add_object({
            let mut d = Dictionary::new();
            d.set("Type", "Outlines");
            d.set("Count", 0);
            d
        });
        let page_id = *doc.get_pages().values().next().unwrap();
        if let Ok(page) = doc.get_object_mut(page_id).and_then(Object::as_dict_mut) {
            page.set("Contents", contents_id);
        }
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        if let Ok(catalog) = doc.get_object_mut(root).and_then(Object::as_dict_mut) {
            catalog.set("Outlines", outlines_id);
        }

        let merged = merge(vec![(doc, vec![1])]).unwrap();
        let page_id = *merged.get_pages().values().next().unwrap();
        let page = merged.get_object(page_id).unwrap().as_dict().unwrap();
        let contents_ref = page.get(b"Contents").unwrap().as_reference().unwrap();
        assert!(
            merged.get_object(contents_ref).is_ok(),
            "content stream kept"
        );

        let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
        assert!(catalog.get(b"Outlines").is_err(), "/Outlines dropped");
    }

    #[test]
    fn inherited_media_box_is_flattened_onto_pages() {
        let merged = merge(vec![(doc_with_inherited_box(2, 70), vec![1, 2])]).unwrap();
        for &id in merged.get_pages().values() {
            let dict = merged.get_object(id).unwrap().as_dict().unwrap();
            let mb = dict.get(b"MediaBox").unwrap().as_array().unwrap();
            assert_eq!(mb[2].as_i64().unwrap(), 70);
        }
        // ...and the rebuilt /Pages node no longer carries the inherited box.
        let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
        let pages_id = catalog.get(b"Pages").unwrap().as_reference().unwrap();
        let pages = merged.get_object(pages_id).unwrap().as_dict().unwrap();
        assert!(pages.get(b"MediaBox").is_err());
    }

    #[test]
    fn empty_selection_is_an_error() {
        assert!(merge(vec![]).is_err());
        assert!(merge(vec![(doc_with_widths(&[10, 20]), vec![])]).is_err());
    }

    #[test]
    fn info_dictionary_is_carried_over() {
        let mut doc = doc_with_widths(&[10]);
        let mut info = Dictionary::new();
        info.set("Producer", Object::string_literal("pdfcat-test"));
        let info_id = doc.add_object(info);
        doc.trailer.set("Info", info_id);

        let merged = merge(vec![(doc, vec![1])]).unwrap();
        let info_ref = merged.trailer.get(b"Info").unwrap().as_reference().unwrap();
        let info = merged.get_object(info_ref).unwrap().as_dict().unwrap();
        assert_eq!(
            info.get(b"Producer").unwrap().as_str().unwrap(),
            b"pdfcat-test"
        );
    }
}
