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
