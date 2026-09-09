// the comment stays on the `(` line, and the head is forced open around it
with ( // glued
	scope
) {
	fn();
}

// an author blank below the glued comment survives
with ( // glued

	scope
) {
	fn();
}

// a block on the `(` line collapses inline in both formatters
with (/* c */ scope) {
	fn();
}
