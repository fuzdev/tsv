# module_keyword

A namespace spelled with the `module` keyword is refused (`UnsupportedKind::ValueNamespace`) even when it holds only types, as both oracles refuse it; the span is the declaration. Spelled `namespace`, the same declaration blanks (`erase/namespace_types_only`).
