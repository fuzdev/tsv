//! Every `tsv_lang` item a language crate re-exports, named through that crate's own path
//! from outside it — cargo-semver-checks cannot see a re-export dropped or demoted to a
//! private `use`, so this file is what fails when one is. And the three crates hand out ONE
//! `Error` (the bindings rely on it), so converting between their names is the identity.

use std::marker::PhantomData;

macro_rules! names_every_reexport {
    ($krate:ident) => {
        const _: () = {
            let _: PhantomData<$krate::Error> = PhantomData;
            let _: PhantomData<$krate::ErrorKind> = PhantomData;
            let _: PhantomData<$krate::ErrorReport> = PhantomData;
            let _: PhantomData<$krate::Result<()>> = PhantomData;
            let _: PhantomData<$krate::WirePoint> = PhantomData;
            let _: usize = $krate::STACK_SIZE;
            let _ = $krate::run_with_stack::<fn(), ()>;
            // plumbing: a `#[doc(hidden)]` item, outside semver, named here because tsv's
            // tooling reaches it through this path
            let _ = $krate::WIRE_COORDINATES;
        };
    };
}

names_every_reexport!(tsv_ts);
names_every_reexport!(tsv_css);
names_every_reexport!(tsv_svelte);

#[test]
fn the_language_crates_share_one_error_type() {
    fn css_from_ts(error: tsv_ts::Error) -> tsv_css::Error {
        error
    }
    fn svelte_from_css(error: tsv_css::Error) -> tsv_svelte::Error {
        error
    }
    let error = tsv_ts::format("let x = (").expect_err("the source must not parse");
    assert_eq!(svelte_from_css(css_from_ts(error.clone())), error);
}

/// `tsv_svelte` hands out `tsv_ts`'s import records: one type under both names.
#[test]
fn the_svelte_crate_reexports_the_import_records() {
    fn svelte_from_ts(imports: tsv_ts::Imports) -> tsv_svelte::Imports {
        imports
    }
    let _: PhantomData<(
        tsv_svelte::ByteSpan,
        tsv_svelte::ImportPhase,
        tsv_svelte::ImportRecord,
        tsv_svelte::ImportRecordKind,
        tsv_svelte::TypeOnly,
    )> = PhantomData;
    let imports = tsv_ts::parse_imports("").expect("an empty source parses");
    assert_eq!(svelte_from_ts(imports.clone()), imports);
}
