//! End-to-end lib-base integration: parse + bind real (small) `.d.ts` lib sources,
//! fold them into a [`LibBase`], and check a program against it — the full S5 path
//! (`bind_lib` -> `LibBase::build` -> `check_program_with_lib`) a single move.
//!
//! Distinct from the `merge` unit tests (which drive synthetic `FileMerge`s): these
//! drive the *binder* over real lib TypeScript, so the lib global's flags come from
//! the same bind path the harness uses.

use bumpalo::Bump;
use tsv_check::{CheckOptions, FileId, LibBase, SourceUnit, bind_lib, check_program_with_lib};

/// `var eval;` conflicts with the lib's `declare function eval` — the
/// `variableDeclarationInStrictMode1` shape, end to end.
#[test]
fn var_eval_conflicts_with_lib_function_eval() {
    let es5 =
        bind_lib("lib.es5.d.ts", "declare function eval(x: string): any;").expect("lib parses");
    let base = LibBase::build(&[&es5]);

    let arena = Bump::new();
    let units = [SourceUnit::new("t.ts", "\"use strict\";\nvar eval;")];
    let result = check_program_with_lib(&units, Some(&base), &arena, &CheckOptions::default());

    // The observable primary is on the test file (FileId 0); the lib-file primary
    // (FileId 1 = lib.es5.d.ts) is present too but is what the baseline masks.
    let test_primary = result
        .diagnostics
        .iter()
        .find(|d| d.file == Some(FileId(0)) && d.code == 2300)
        .expect("a TS2300 on the test file");
    // `var eval` starts on line 2, column 5 -> byte offset of the `eval` name.
    assert_eq!(test_primary.related.len(), 1);
    assert_eq!(test_primary.related[0].code, 6203);
    assert_eq!(test_primary.related[0].file, Some(FileId(1))); // lib.es5.d.ts
    // A masked lib-file primary exists (the runner drops it; the baseline hides it).
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.file == Some(FileId(1)) && d.code == 2300)
    );
}

/// `class Promise<T> {}` conflicts with a lib global declared across several files,
/// producing the priority-ordered TS6203/6204 related chain.
#[test]
fn class_promise_conflicts_across_lib_files() {
    // Priority order: es5 (interface), es2015.iterable (interface), es2015.promise
    // (var) — the fold order fixes the related-info attribution.
    let es5 = bind_lib("lib.es5.d.ts", "interface Promise<T> {}").expect("parses");
    let iterable = bind_lib("lib.es2015.iterable.d.ts", "interface Promise<T> {}").expect("parses");
    let promise = bind_lib(
        "lib.es2015.promise.d.ts",
        "declare var Promise: PromiseConstructor;",
    )
    .expect("parses");
    let base = LibBase::build(&[&es5, &iterable, &promise]);

    let arena = Bump::new();
    let units = [SourceUnit::new(
        "promiseDefinitionTest.ts",
        "class Promise<T> {}",
    )];
    let result = check_program_with_lib(&units, Some(&base), &arena, &CheckOptions::default());

    let primary = result
        .diagnostics
        .iter()
        .find(|d| d.file == Some(FileId(0)) && d.code == 2300)
        .expect("a TS2300 on the test file");
    let codes: Vec<u32> = primary.related.iter().map(|r| r.code).collect();
    assert_eq!(codes, vec![6203, 6204, 6204]);
    // Priority order: es5 (FileId 1), es2015.iterable (2), es2015.promise (3).
    let files: Vec<Option<FileId>> = primary.related.iter().map(|r| r.file).collect();
    assert_eq!(
        files,
        vec![Some(FileId(1)), Some(FileId(2)), Some(FileId(3))]
    );
}

/// A clean augmentation (`interface Array<T> {}` merging into the lib's `Array`)
/// emits nothing — the lib base must not manufacture spurious conflicts.
#[test]
fn interface_augmentation_of_lib_is_silent() {
    let es5 = bind_lib(
        "lib.es5.d.ts",
        "interface Array<T> { length: number; }\ndeclare var Array: ArrayConstructor;",
    )
    .expect("parses");
    let base = LibBase::build(&[&es5]);

    let arena = Bump::new();
    let units = [SourceUnit::new(
        "t.ts",
        "interface Array<T> { extra(): void; }",
    )];
    let result = check_program_with_lib(&units, Some(&base), &arena, &CheckOptions::default());
    assert!(
        result.diagnostics.is_empty(),
        "a legal interface merge must be silent"
    );
}

/// With no lib base, the same program is clean (the conflict is lib-sourced) —
/// proving the base is what introduces the cross-declaration-space conflict.
#[test]
fn no_lib_base_no_conflict() {
    let arena = Bump::new();
    let units = [SourceUnit::new("t.ts", "var eval;")];
    let result = check_program_with_lib(&units, None, &arena, &CheckOptions::default());
    assert!(result.diagnostics.is_empty());
}
