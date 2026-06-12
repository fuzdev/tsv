// Block comment on own line, parens on next line
a.b(
  [c],
  /* block */
  (d.eee(fff, ggg)),
);

// JSDoc comment on own line, parens on next line
a.b(
  [c],
  /** @type {T} */
  (d.eee(fff, ggg)),
);

// Same-line block comment before parens (should stay inline)
a.b(
  [c],
  /* block */ (d.eee(fff, ggg)),
);
