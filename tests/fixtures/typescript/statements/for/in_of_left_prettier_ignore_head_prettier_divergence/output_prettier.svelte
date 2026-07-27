<script lang="ts">
	// an own-line directive in the `for (`→left gap freezes the whole left clause
	for (// prettier-ignore
	const  xxx; of yyy) {
		fn();
	}

	// for-in shares the head, so it shares the freeze
	for (// prettier-ignore
	const  kkk; in obj) {
		fn();
	}

	// a left that is a destructuring pattern freezes whole
	for (// prettier-ignore
	[ aaa , bbb ] of yyy) {
		fn();
	}

	// an own-line block comment behaves identically — placement keys the freeze, and holds
	// the header open so the directive keeps the line that makes it honored
	for (/* prettier-ignore */
	const  zzz; of yyy) {
		fn();
	}

	// the same placement rule in the keyword→binding gap, where the header breaks for
	// either spelling and the binding normalizes
	for (const /* prettier-ignore */
		[aaa, bbb] of yyy) {
		fn();
	}
</script>
