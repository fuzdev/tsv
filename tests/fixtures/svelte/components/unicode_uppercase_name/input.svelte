<!-- A non-ASCII uppercase initial makes a component (self-closes like PascalCase) -->
<Δcomp />

<Écomp />

<Яcomp />

<!-- With props -->
<Δcomp prop="value" />

<!-- With children — component layout (boundary breaks preserved) -->
<Δcomp>
	<span>text</span>
</Δcomp>
