<script lang="ts">
	// an own-line directive in the `label:`→body gap freezes the body statement
	lll:
	// prettier-ignore
	for (;;) {
		fn(  bbb  );
		break lll;
	}

	// a block body is the same head
	mmm:
	// prettier-ignore
	{
		fn(  ccc  );
	}

	// an own-line block comment behaves identically — placement keys the freeze
	nnn:
	/* prettier-ignore */
	while (aaa) {
		fn(  ddd  );
		break nnn;
	}
</script>
