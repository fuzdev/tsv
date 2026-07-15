# Divergence: meta-property dot-gap line comments (preserve)

A *line* comment in either gap around a meta property's `.` (`new // c⏎.target`,
`import // c⏎.meta`). tsv keeps it where the author wrote it and continues `.property` one level
down; prettier **relocates** it — for `new.target`, all the way past the `;`.

```ts
// tsv (preserve)          // prettier (relocate)
return new // c1           return new.target; // c1
	.target;
```

The comment moves from *inside* the meta property to *after the whole statement*, so it no longer
reads as being about the `.target` at all. On `import.meta` it lands differently again
(`import.meta // c2⏎.url;`) — the same authored position, two different destinations, decided by
what follows.

Block comments in these gaps are **not** a divergence: prettier keeps each on its authored side of
the dot, and so does tsv — pinned by the regular sibling [dot_gap_comments](../dot_gap_comments/).

**Why this fixture exists:** tsv **dropped** every comment in both gaps. `build_meta_property_doc`
concatenated `meta` + `"."` + `property` and scanned neither gap — the same class as a comment
inside a multi-word keyword ([§Comments inside a multi-word keyword](../../../../../../../docs/conformance_prettier.md#comments-inside-a-multi-word-keyword)),
and the case that shows why that class's detector (a `d.text` literal with an *interior* space) is
only a proxy: a header joined by a punctuator has no space to find.

See [conformance_prettier.md §Comment relocation](../../../../../../../docs/conformance_prettier.md#comment-relocation).
