<script lang="ts">
	// When a call has a multiline-content argument (here a multiline template), the
	// args route through the shared joined-args path. A non-last argument carrying a
	// block comment before the comma and a line comment after it must keep both — the
	// block was dropped when a line comment coexisted.
	fn(
		a /* c1 */, // c2
		`line1
line2`,
	);

	// Same path for `new`.
	new A(
		a /* c1 */, // c2
		`line1
line2`,
	);
</script>
