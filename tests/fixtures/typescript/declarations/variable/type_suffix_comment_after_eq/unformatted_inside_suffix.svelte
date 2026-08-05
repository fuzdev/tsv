<script lang="ts">
	// a comment in the annotation's trailing gap escapes the type and lands after the
	// `=`, dropping the initializer to its own line
	// (unformatted_inside_suffix carries the authored placement)
	const a1: T[K // c1
	] = y;

	// a redundant paren's trailing gap escapes the same way once the parens are stripped
	const a2: (A // c2
	) = y;

	// with no initializer there is no `=` to cross — the comment trails the statement
	let b: T[K // c3
	];
</script>
