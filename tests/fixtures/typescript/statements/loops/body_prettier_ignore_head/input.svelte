<script lang="ts">
	// an own-line directive in a loop's `)`→body gap freezes the body statement
	while (aaa)
		// prettier-ignore
		fn(  bbb  );

	// a block body is the same head — the braces ride inside the frozen slice
	while (aaa)
	// prettier-ignore
	{
		fn(  ccc  );
	}

	// a C-style `for` header is the same head
	for (;;)
		// prettier-ignore
		fn(  ddd  );

	// so is a for-of header
	for (const kkk of yyy)
	// prettier-ignore
	{
		fn(  eee  );
	}

	// and a for-in header
	for (const nnn in obj)
		// prettier-ignore
		fn(  fff  );

	// `do` introduces its body directly, with no `)` of its own
	do
		// prettier-ignore
		fn(  ggg  );
	while (aaa);

	// an own-line block comment behaves identically — placement keys the freeze
	while (aaa)
		/* prettier-ignore */
		fn(  hhh  );
</script>
