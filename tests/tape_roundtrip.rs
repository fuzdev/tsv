//! The tape transcoder (`tsv_arena::tape::Transcoder`, `docs/tape_format.md`) over every fixture
//! document's span-only wire: the tape prints back (`tsv_debug::tape::tape_to_json`) to the wire
//! byte for byte, so the transcoder drops nothing the decoder would need to rebuild the tree —
//! no number's spelling, no string, no key. `npm run check:tape` grades the same tapes against the
//! offline JS encoder's and through the shipped decoder.
//!
//! The CSS wire writer's direct tape (its tape copy, `tsv_css::__private::write_tape`) is graded
//! over every CSS document the fixture tree holds — each `.css` input and each `<style>` body —
//! against the same parse's JSON wire: it prints back to the wire, and the transcoder makes the
//! same tape of the printback as of the wire (`tsv_debug::tape::grade_css_direct_tape`).

use std::path::Path;

use tsv_arena::tape::Transcoder;
use tsv_debug::tape::{
    bytes_difference, fixture_wires, grade_css_direct_tape, style_bodies, tape_to_json, wire_source,
};
use tsv_lang::tape::TapeSink;

#[test]
fn every_fixture_wire_transcodes_and_prints_back() {
    let mut transcoder = Transcoder::new();
    let mut failures = Vec::new();
    let graded = fixture_wires(Path::new("tests/fixtures"), |doc| {
        let source = wire_source(doc.language, &doc.source);
        let tape = match transcoder.transcode(&doc.span, source) {
            Ok(tape) => tape.to_vec(),
            Err(e) => {
                failures.push(format!("{}: transcode: {e}", doc.path));
                return;
            }
        };
        match tape_to_json(&tape, source) {
            Ok(back) => {
                if let Some(d) = bytes_difference(&back, &doc.span) {
                    failures.push(format!("{}: prints back differently {d}", doc.path));
                }
            }
            Err(e) => failures.push(format!("{}: print: {e}", doc.path)),
        }
    })
    .expect("the fixture tree walks");
    assert!(graded > 0, "no fixture document was graded");
    assert!(
        failures.is_empty(),
        "{} of {graded} documents:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A real source whose wire refits at every level: Svelte attaches each `/*c*/` as the
/// `leadingComments` of a `TSTypeAnnotation`, a key after its `typeAnnotation` subtree, so the
/// first candidate entry fails only at the close and the object is rewound to the one that
/// carries the comment lists. Each level rewinds once; a rewind of an enclosing level re-reads a
/// settled one at its settled entry, never from its first candidate again — which the transcoder
/// once did, doubling the work per level.
#[test]
fn nested_late_refits_from_a_real_source_rewind_once_per_level() {
    // shallow on purpose: an exponential regression must fail the bound, not hang the run
    let depth = 16;
    let mut ty = String::from("T");
    for k in (0..depth).rev() {
        ty = format!("(x{k} /*c*/: {ty}) => T");
    }
    let source = format!("<script lang=\"ts\">let a: {ty};</script>");
    let ast = tsv_svelte::__private::parse(&source).expect("parses");
    let span = tsv_svelte::__private::convert_ast_json_bytes(&ast, &source);
    let mut transcoder = Transcoder::new();
    let tape = transcoder
        .transcode(&span, wire_source("svelte", &source))
        .expect("transcodes")
        .to_vec();
    let refits = transcoder.refits();
    assert!(
        (depth..=2 * depth).contains(&refits),
        "{refits} rewinds for {depth} levels: fewer than {depth} means the source no longer forces a \
         refit per level, more than {} means the rewinds grew superlinear",
        2 * depth
    );
    let back = tape_to_json(&tape, wire_source("svelte", &source)).expect("prints");
    assert_eq!(bytes_difference(&back, &span), None);
}

/// The CSS writer's direct tape over every CSS document of the fixture tree: each `.css`
/// document, and each `<style>` body of a Svelte one read as a stylesheet of its own.
#[test]
fn every_fixture_css_document_writes_its_tape_directly() {
    let mut transcoder = Transcoder::new();
    let mut sink = TapeSink::new();
    let mut failures = Vec::new();
    let (mut files, mut bodies) = (0usize, 0usize);
    let mut grade = |path: &str, source: &str, json: &[u8]| {
        if let Err(e) = grade_css_direct_tape(source, json, &mut transcoder, &mut sink) {
            failures.push(format!("{path}: {e}"));
        }
    };
    fixture_wires(Path::new("tests/fixtures"), |doc| match doc.language {
        "css" => {
            files += 1;
            grade(&doc.path, &doc.source, &doc.span);
        }
        "svelte" => {
            for body in style_bodies(&doc.source) {
                let Ok(ast) = tsv_css::__private::parse(body) else {
                    continue;
                };
                bodies += 1;
                let json = tsv_css::__private::convert_ast_json_bytes(&ast, body);
                grade(&format!("{} <style>", doc.path), body, &json);
            }
        }
        _ => {}
    })
    .expect("the fixture tree walks");
    // floors well under today's counts: a walk that stopped finding CSS must fail, not pass
    assert!(files >= 5, "only {files} .css documents graded");
    assert!(bodies >= 1000, "only {bodies} <style> bodies graded");
    assert!(
        failures.is_empty(),
        "{} of {} CSS documents:\n{}",
        failures.len(),
        files + bodies,
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
