use super::*;

/// The decompressed `/Contents` stream bytes of every page, in page
/// order. Used as a page-identity fingerprint that survives `merge`
/// (which carries content streams byte-for-byte).
fn page_content_bytes(doc: &Document) -> Vec<Vec<u8>> {
    doc.get_pages()
        .values()
        .map(|&id| page_contents_one(doc, id))
        .collect()
}

fn page_contents_one(doc: &Document, page_id: ObjectId) -> Vec<u8> {
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    let contents_id = page.get(b"Contents").unwrap().as_reference().unwrap();
    let mut stream = doc
        .get_object(contents_id)
        .unwrap()
        .as_stream()
        .unwrap()
        .clone();
    let _ = stream.decompress();
    stream.content
}

fn page_ids(doc: &Document) -> Vec<ObjectId> {
    doc.get_pages().values().copied().collect()
}

#[test]
fn concatenates_pages_in_order() {
    let one = Document::load("tests/fixtures/1page.pdf").unwrap();
    let three = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let expected = [
        page_contents_one(&one, one.get_pages()[&1]),
        page_contents_one(&three, three.get_pages()[&1]),
        page_contents_one(&three, three.get_pages()[&2]),
        page_contents_one(&three, three.get_pages()[&3]),
    ];

    let merged = merge(vec![
        (
            "1page.pdf".to_string(),
            Document::load("tests/fixtures/1page.pdf").unwrap(),
            vec![1],
        ),
        (
            "3pages.pdf".to_string(),
            Document::load("tests/fixtures/3pages.pdf").unwrap(),
            vec![1, 2, 3],
        ),
    ])
    .unwrap();
    assert_eq!(page_content_bytes(&merged), expected);
    let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
    let pages_id = catalog.get(b"Pages").unwrap().as_reference().unwrap();
    let pages = merged.get_object(pages_id).unwrap().as_dict().unwrap();
    assert_eq!(pages.get(b"Count").unwrap().as_i64().unwrap(), 4);
}

#[test]
fn selects_and_reorders_pages() {
    let src = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let expected = [
        page_contents_one(&src, src.get_pages()[&3]),
        page_contents_one(&src, src.get_pages()[&1]),
        page_contents_one(&src, src.get_pages()[&2]),
    ];
    let merged = merge(vec![(
        "src.pdf".to_string(),
        Document::load("tests/fixtures/3pages.pdf").unwrap(),
        vec![3, 1, 2],
    )])
    .unwrap();
    assert_eq!(page_content_bytes(&merged), expected);
}

#[test]
fn duplicate_page_gets_a_fresh_id() {
    let src = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let p1 = page_contents_one(&src, src.get_pages()[&1]);
    let p2 = page_contents_one(&src, src.get_pages()[&2]);
    let merged = merge(vec![(
        "src.pdf".to_string(),
        Document::load("tests/fixtures/3pages.pdf").unwrap(),
        vec![1, 1, 2],
    )])
    .unwrap();
    assert_eq!(page_content_bytes(&merged), [p1.clone(), p1, p2]);
    let ids = page_ids(&merged);
    let unique: HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), 3, "every page must be a distinct object");
}

#[test]
fn duplicate_then_more_inputs_keeps_ids_disjoint() {
    // A duplicated page in the first input must not steal an id that the
    // second input's objects will be renumbered onto.
    let three = Document::load("tests/fixtures/3pages.pdf").unwrap();
    let one = Document::load("tests/fixtures/1page.pdf").unwrap();
    let p1 = page_contents_one(&three, three.get_pages()[&1]);
    let p_one = page_contents_one(&one, one.get_pages()[&1]);
    let merged = merge(vec![
        (
            "a.pdf".to_string(),
            Document::load("tests/fixtures/3pages.pdf").unwrap(),
            vec![1, 1],
        ),
        (
            "b.pdf".to_string(),
            Document::load("tests/fixtures/1page.pdf").unwrap(),
            vec![1, 1],
        ),
    ])
    .unwrap();
    assert_eq!(
        page_content_bytes(&merged),
        [p1.clone(), p1, p_one.clone(), p_one]
    );
    let unique: HashSet<_> = page_ids(&merged).into_iter().collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn keeps_supporting_objects_but_drops_outlines() {
    // with-outline.pdf has /Outlines (from hyperref's \section) and real
    // page content streams. After merging, content streams must survive
    // and /Outlines must not.
    let merged = merge(vec![(
        "with-outline.pdf".to_string(),
        Document::load("tests/fixtures/with-outline.pdf").unwrap(),
        vec![1, 2],
    )])
    .unwrap();

    // Every merged page still has a usable /Contents reference.
    for &id in merged.get_pages().values() {
        let page = merged.get_object(id).unwrap().as_dict().unwrap();
        let contents_ref = page.get(b"Contents").unwrap().as_reference().unwrap();
        assert!(
            merged.get_object(contents_ref).is_ok(),
            "content stream kept"
        );
    }

    let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
    assert!(catalog.get(b"Outlines").is_err(), "/Outlines dropped");
}

#[test]
fn inheritable_attrs_flattened_and_stale_keys_stripped() {
    // inherited-resources.pdf carries /Resources on its /Pages node;
    // leaves do not have /Resources. After merge, (a) each leaf must
    // carry the flattened /Resources, and (b) the rebuilt /Pages node
    // must no longer carry /Resources (STALE_PAGES_KEYS strip).
    let merged = merge(vec![(
        "inherited-resources.pdf".to_string(),
        Document::load("tests/fixtures/inherited-resources.pdf").unwrap(),
        vec![1, 2],
    )])
    .unwrap();

    for &id in merged.get_pages().values() {
        let page = merged.get_object(id).unwrap().as_dict().unwrap();
        assert!(
            page.get(b"Resources").is_ok(),
            "leaf must have /Resources after flattening"
        );
    }

    let root = merged.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let catalog = merged.get_object(root).unwrap().as_dict().unwrap();
    let pages_id = catalog.get(b"Pages").unwrap().as_reference().unwrap();
    let pages = merged.get_object(pages_id).unwrap().as_dict().unwrap();
    assert!(
        pages.get(b"Resources").is_err(),
        "/Pages node must not retain stale /Resources"
    );
}

#[test]
#[should_panic(expected = "merge called with no sources")]
fn merge_with_no_sources_panics() {
    let _: Result<_, _> = merge(vec![]);
}

#[test]
fn info_dictionary_is_carried_over() {
    // 1page.pdf has /Info (LuaTeX writes /Producer etc.).
    let merged = merge(vec![(
        "1page.pdf".to_string(),
        Document::load("tests/fixtures/1page.pdf").unwrap(),
        vec![1],
    )])
    .unwrap();
    let info_ref = merged.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let info = merged.get_object(info_ref).unwrap().as_dict().unwrap();
    // Don't assert the exact producer string (toolchain-dependent);
    // just that /Producer is present.
    assert!(info.get(b"Producer").is_ok());
}

#[test]
fn no_catalog_error_carries_first_input_path() {
    // A document missing /Root in the trailer trips skeleton extraction.
    let doc = Document::with_version("1.5");
    // No Root set on trailer.
    let err = merge(vec![("missing-root.pdf".to_string(), doc, vec![])]).unwrap_err();
    match err {
        MergeError::NoCatalog { path } => assert_eq!(path, "missing-root.pdf"),
        other => panic!("expected NoCatalog, got {other:?}"),
    }
}

#[test]
fn empty_selection_yields_zero_page_document() {
    // With NoPages removed, asking for zero pages from a real document
    // now produces a valid 0-page merged document. This path is
    // unreachable from the CLI (parse_ranges rejects empty specs, and
    // load_one_source rejects 0-page inputs), but the in-module
    // contract is pinned here.
    let merged = merge(vec![(
        "3pages.pdf".to_string(),
        Document::load("tests/fixtures/3pages.pdf").unwrap(),
        vec![],
    )])
    .unwrap();
    assert_eq!(merged.get_pages().len(), 0);
}

#[test]
fn picks_highest_pdf_version() {
    // 3pages.pdf is PDF 1.7 (TeX Live 2026 default); v1.4.pdf is PDF
    // 1.4 (forced via \pdfvariable minorversion=4). merge should pick 1.7.
    let merged = merge(vec![
        (
            "3pages.pdf".to_string(),
            Document::load("tests/fixtures/3pages.pdf").unwrap(),
            vec![1],
        ),
        (
            "v1.4.pdf".to_string(),
            Document::load("tests/fixtures/v1.4.pdf").unwrap(),
            vec![1],
        ),
    ])
    .unwrap();
    assert_eq!(merged.version, "1.7");
}
