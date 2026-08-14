<script lang="ts">
	// a block glued FORWARD to the member does not break an object↔non-object boundary,
	// however the author broke the line before it
	type A = aa &
		// k
		{ x: 1 } & /* c */ cc;

	// the non-object→object direction takes the same answer
	type B = aa &
		// k
		cc & /* c */ { x: 1 };

	// control: the same comment authored on the `&`'s line
	type C = aa &
		// k
		{ x: 1 } & /* c */ cc;
</script>
