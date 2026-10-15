# export_import_equals

`export import A = N.B` is refused (`UnsupportedKind::ImportEquals`) like an unexported import-equals; the span is the declaration, `export` included.
