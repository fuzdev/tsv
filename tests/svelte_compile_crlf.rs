//! The Svelte compiler takes a component's own line endings: its server output is
//! reprinted through `tsv_ts::format_canonical`, which (unlike the intent-preserving
//! format entry points) does not require CR-folded text. A CRLF component must compile
//! — or refuse — in a debug build, never trip the format path's CR-folded-text check.

use tsv_svelte_compile::{CompileOptions, compile};

#[test]
fn a_crlf_component_compiles_without_tripping_the_cr_fold_check() {
    for source in [
        "<script>\r\n\tlet a = 1;\r\n</script>\r\n\r\n<p>{a}</p>\r\n",
        "<script>\r\n\tconst t = `x\r\ny`;\r\n</script>\r\n<p>{t}</p>\r\n",
        "<pre>a\r\nb</pre>\r\n",
        "<p>{1 +\r\n\t2}</p>\r\n",
    ] {
        // a refusal is fine; a panic is what this guards against
        let _ = compile(source, &CompileOptions::default());
    }
}
