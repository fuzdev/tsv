<script lang="ts">
	// an own-line directive in the `}`→`catch` gap freezes the whole catch clause
	try {
		fn(aaa);
	} catch (eee) {
		// prettier-ignore
		fn(  bbb  );
	}

	// the `}`→`finally` gap is the same head
	try {
		fn(aaa);
	} finally {
		// prettier-ignore
		fn(  ccc  );
	}

	// an own-line block comment behaves identically — placement keys the freeze
	try {
		fn(aaa);
	} catch (eee) {
		/* prettier-ignore */
		fn(  ddd  );
	}
</script>
