<script lang="ts">
	// an own-line directive in the `}`→`catch` gap freezes the whole catch clause
	try {
		fn(   aaa   );
	}
	// prettier-ignore
	catch (  eee  ) {
		fn(  bbb  );
	}

	// the `}`→`finally` gap is the same head
	try {
		fn(    aaa    );
	}
	// prettier-ignore
	finally {
		fn(  ccc  );
	}

	// an own-line block comment behaves identically — placement keys the freeze
	try {
		fn(  aaa  );
	}
	/* prettier-ignore */
	catch (eee) {
		fn(  ddd  );
	}
</script>
