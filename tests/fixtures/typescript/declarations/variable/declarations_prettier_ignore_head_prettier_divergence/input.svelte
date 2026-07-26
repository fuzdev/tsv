<script lang="ts">
	// an own-line directive after the keyword freezes the first declarator and keeps
	// its own line
	const
		// prettier-ignore
		aaa   =   1,
		bbb = 2;

	// the block spelling behaves identically
	let
		/* prettier-ignore */
		ccc   =   3,
		ddd = 4;

	// a `for` header's init clause is the same declarator list
	for (
		let
			// prettier-ignore
			eee   =   5;
		;
	) {
		break;
	}
</script>
