//! Generate the synthetic PDF fixtures that LuaLaTeX cannot produce.
//!
//! Run from the crate root:
//!     cargo run --example gen_synthetic_fixtures
//!
//! Outputs:
//!   tests/fixtures/inherited-resources.pdf
//!   tests/fixtures/0page.pdf

use lopdf::{Dictionary, Document, Object};

fn write_inherited_resources(path: &str) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.add_object(Dictionary::new());

    // /Resources lives on the /Pages node — leaves inherit it.
    let mut font = Dictionary::new();
    font.set("Type", "Font");
    font.set("Subtype", "Type1");
    font.set("BaseFont", "Helvetica");
    let font_id = doc.add_object(font);
    let mut fonts = Dictionary::new();
    fonts.set("F1", font_id);
    let mut resources = Dictionary::new();
    resources.set("Font", fonts);

    let kids: Vec<Object> = (0..2)
        .map(|_| {
            let mut page = Dictionary::new();
            page.set("Type", "Page");
            page.set("Parent", pages_id);
            page.set(
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(100),
                    Object::Integer(100),
                ]),
            );
            // NO /Resources on the leaf — it inherits from /Pages.
            Object::Reference(doc.add_object(page))
        })
        .collect();

    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", 2i64);
    pages.set("Kids", kids);
    pages.set("Resources", resources);
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);

    doc.save(path).unwrap();
}

fn write_zero_pages(path: &str) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.add_object(Dictionary::new());
    let mut pages = Dictionary::new();
    pages.set("Type", "Pages");
    pages.set("Count", 0i64);
    pages.set("Kids", Object::Array(vec![]));
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::new();
    catalog.set("Type", "Catalog");
    catalog.set("Pages", pages_id);
    let catalog_id = doc.add_object(catalog);
    doc.trailer.set("Root", catalog_id);

    doc.save(path).unwrap();
}

fn main() {
    write_inherited_resources("tests/fixtures/inherited-resources.pdf");
    write_zero_pages("tests/fixtures/0page.pdf");
    println!("Wrote tests/fixtures/inherited-resources.pdf");
    println!("Wrote tests/fixtures/0page.pdf");
}
