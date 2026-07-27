// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The fixture cases themselves — one divergence class each, by construction.
//!
//! Split from `fixtures.rs` so the container-building machinery and the list of
//! what is being claimed stay separately readable. The claims are the part a
//! reviewer has to check against the taxonomy; the builder is the part that has
//! to be boring.

use crate::error::Result;
use crate::fixtures::{
    CHAPTER, Fixture, OPF, base, base_plan, build, lie_about_size, mutate, plan,
};
use crate::taxonomy::Class;

/// Every fixture. Each asserts exactly one class.
pub fn all() -> Result<Vec<Fixture>> {
    let base = base()?;
    let mut out = Vec::new();

    // --- A: entry content -------------------------------------------------
    out.push(Fixture {
        id: "content-byte-diff",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                p.body = CHAPTER.replace("Hello.", "Hello!").into_bytes();
            }),
            None,
        )?,
        expect: &[Class::ContentByteDiff],
    });

    out.push(Fixture {
        id: "entry-missing",
        source: base.clone(),
        roundtrip: build(
            &base_plan()
                .into_iter()
                .filter(|p| p.name != "OEBPS/ch1.xhtml")
                .collect::<Vec<_>>(),
            None,
        )?,
        // Dropping a manifested file *is* a manifest mismatch: the OPF still
        // declares `ch1.xhtml` and the container no longer has it. Both are
        // true, and neither can happen without the other.
        expect: &[Class::EntryMissing, Class::ManifestMismatch],
    });

    let mut added = base_plan();
    added.push(plan("OEBPS/extra.txt", b"surplus".to_vec()));
    out.push(Fixture {
        id: "entry-added",
        source: base.clone(),
        roundtrip: build(&added, None)?,
        expect: &[Class::EntryAdded],
    });

    // Attribute order reversed and the empty element expanded: same infoset,
    // different bytes.
    out.push(Fixture {
        id: "xml-canonicalization",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                p.body = CHAPTER
                    .replace("id=\"a\" class=\"x\"", "class=\"x\" id=\"a\"")
                    .into_bytes();
            }),
            None,
        )?,
        expect: &[Class::XmlCanonicalization],
    });

    out.push(Fixture {
        id: "text-encoding",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                let mut b = vec![0xEF, 0xBB, 0xBF];
                b.extend_from_slice(CHAPTER.as_bytes());
                p.body = b;
            }),
            None,
        )?,
        expect: &[Class::TextEncoding],
    });

    out.push(Fixture {
        id: "line-endings",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                p.body = CHAPTER.replace('\n', "\r\n").into_bytes();
            }),
            None,
        )?,
        expect: &[Class::LineEndings],
    });

    // --- B: container metadata --------------------------------------------
    let mut reordered = base_plan();
    reordered.swap(3, 4);
    out.push(Fixture {
        id: "entry-order",
        source: base.clone(),
        roundtrip: build(&reordered, None)?,
        expect: &[Class::EntryOrder],
    });

    out.push(Fixture {
        id: "compression-method",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| p.stored = true),
            None,
        )?,
        expect: &[Class::CompressionMethod],
    });

    out.push(Fixture {
        id: "comment",
        source: base.clone(),
        roundtrip: build(&base_plan(), Some("written by something else"))?,
        expect: &[Class::Comment],
    });

    // --- C: declared versus actual ----------------------------------------
    let mut not_first = base_plan();
    not_first.swap(0, 1);
    out.push(Fixture {
        id: "mimetype-not-first",
        source: base.clone(),
        roundtrip: build(&not_first, None)?,
        // Moving mimetype out of first position *is* a reordering. Both are true.
        expect: &[Class::EntryOrder, Class::MimetypeNotFirst],
    });

    out.push(Fixture {
        id: "mimetype-compressed",
        source: base.clone(),
        roundtrip: build(&mutate(base_plan(), "mimetype", |p| p.stored = false), None)?,
        // Compressing mimetype *is* a compression-method change. Both are true.
        expect: &[Class::CompressionMethod, Class::MimetypeCompressed],
    });

    // A zip extra field on one side only. `add_extra_data` writes it to the
    // local header, which is where `Archive::read` looks.
    out.push(Fixture {
        id: "extra-field",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                // 0x5455 is the extended-timestamp field: flags byte, then a
                // 4-byte modification time. Common enough that a writer
                // dropping it is a real fidelity loss.
                p.extra = Some((0x5455, vec![0x01, 0x00, 0x00, 0x00, 0x00]));
            }),
            None,
        )?,
        expect: &[Class::ExtraField],
    });

    // Both sides carry the lie, because `DeclaredSizeMismatch` is checked
    // against the round-trip alone — it is a claim the container makes about
    // itself, not a difference between two containers. Patching one side would
    // add a pairwise divergence that has nothing to do with the class.
    let lying = lie_about_size(&base, "OEBPS/ch1.xhtml", 9_999);
    out.push(Fixture {
        id: "declared-size-mismatch",
        source: lying.clone(),
        roundtrip: lying,
        expect: &[Class::DeclaredSizeMismatch],
    });

    // Same reasoning: a manifest that names a file nobody wrote is a property
    // of one container. Declaring the phantom on both sides isolates the class
    // from the `content-byte-diff` that changing only one OPF would produce.
    let phantom = build(
        &mutate(base_plan(), "OEBPS/package.opf", |p| {
            p.body = OPF
                .replace(
                    "</manifest>",
                    "<item id=\"gone\" href=\"missing.xhtml\"                      media-type=\"application/xhtml+xml\"/></manifest>",
                )
                .into_bytes();
        }),
        None,
    )?;
    out.push(Fixture {
        id: "manifest-mismatch",
        source: phantom.clone(),
        roundtrip: phantom,
        expect: &[Class::ManifestMismatch],
    });

    // --- The identity case, which is the one that catches a broken instrument.
    out.push(Fixture {
        id: "identical",
        source: base.clone(),
        roundtrip: base,
        expect: &[], // the case that catches an instrument crying wolf
    });

    Ok(out)
}
