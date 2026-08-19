<script lang="ts">
	// A line comment retains a shell the frozen value does not otherwise need: inline it
	// would swallow the `)`, and the clause's `;` is not a terminator it may defer past.
	for (
		let aaa =
			// prettier-ignore
			(
				bbb  +  ccc // c
			);
		aaa;
		aaa
	) {
		fn();
	}
</script>
