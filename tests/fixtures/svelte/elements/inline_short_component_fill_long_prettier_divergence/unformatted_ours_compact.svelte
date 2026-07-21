<!-- A SHORT inline component (its own content fits) sandwiched in a wrapping fill of text and inline
     components packs greedily and pairwise after the preceding word rather than being isolated onto
     its own line: <Comp>example.dev</Comp> stays on the <Comp>preferences</Comp> run line. The
     component sibling of inline_short_content_fill_long (HTML <code>) — components route through a
     different classification but the same after-element fold, so they pack identically. Prettier
     keeps the run on one over-width line (the inline-content hug); tsv keeps printWidth a hard limit
     and wraps at a word boundary. The two blocks pin the 100/101 boundary: the run packs to exactly
     100 (`more` fits, `text` breaks), and one char more pushes `more` to 101 so it breaks instead.
     See README. -->
<p>
	alpha bravo charlie delta echo foxtrot golf hotel india
	<Comp>preferences</Comp> and set <Comp>example.dev</Comp> as one exception here plus a little more text
</p>

<p>
	alpha bravo charlie delta echo foxtrot golf hotel india
	<Comp>preferences</Comp> and sett <Comp>example.dev</Comp> as one exception here plus a little more text
</p>
