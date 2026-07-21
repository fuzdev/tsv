<!-- A SHORT inline element (its own content fits) sandwiched in a wrapping fill of text and inline
     elements packs greedily and pairwise after the preceding word rather than being isolated onto
     its own line: <code>example.dev</code> stays on the <code>preferences</code> run line. Prettier
     keeps the run on one over-width line (the inline-content hug); tsv keeps printWidth a hard limit
     and wraps at a word boundary. The two blocks pin the 100/101 boundary: the run packs to exactly
     100 (`more` fits, `text` breaks), and one char more pushes `more` to 101 so it breaks instead.
     See README. -->
<p>
	alpha bravo charlie delta echo foxtrot golf hotel india
	<code>preferences</code> and set <code>example.dev</code> as one exception here plus a little more text
</p>

<p>
	alpha bravo charlie delta echo foxtrot golf hotel india
	<code>preferences</code> and sett <code>example.dev</code> as one exception here plus a little more text
</p>
