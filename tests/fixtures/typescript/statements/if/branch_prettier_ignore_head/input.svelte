<script lang="ts">
	// an own-line directive in the `)`→consequent gap freezes the consequent statement
	if (aaa)
		// prettier-ignore
		fn(  bbb  );

	// the `else`→alternate gap is the same head
	if (aaa) {
		fn(bbb);
	} else
		// prettier-ignore
		fn(  ccc  );

	// an `else if` alternate is a statement like any other, and freezes whole
	if (aaa) {
		fn(bbb);
	} else
		// prettier-ignore
		if (ccc) {
			fn(  ddd  );
		}

	// an own-line block comment behaves identically — placement keys the freeze
	if (aaa)
		/* prettier-ignore */
		fn(  bbb  );
</script>
