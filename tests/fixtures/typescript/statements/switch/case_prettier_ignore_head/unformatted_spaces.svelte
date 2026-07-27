<script lang="ts">
	// an own-line directive in the `{`→first-case gap freezes the whole case
	switch (  aaa  ) {
		// prettier-ignore
		case   1:
			fn(  bbb  );
		case    2:
			fn(  ccc  );
	}

	// between cases, the directive freezes the case that follows it
	switch (  aaa  ) {
		case    1:
			fn(   bbb   );
		// prettier-ignore
		case   2:
			fn(  ccc  );
	}

	// a `default` case is the same member
	switch (aaa) {
		case     1:
			fn(  bbb  );
		// prettier-ignore
		default  :
			fn(  ccc  );
	}

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	switch (   aaa   ) {
		/* prettier-ignore */
		case   1:
			fn(  bbb  );
	}

	// a multi-line frozen case keeps its verbatim layout
	switch (  aaa  ) {
		// prettier-ignore
		case 1:
			fn(
				bbb,
					ccc
			);
	}

	// a comment trailing the frozen case's last statement sits outside the slice
	switch (aaa) {
		// prettier-ignore
		case   1:
			fn(  bbb  ); /* c */
		case 2:
			fn(ccc);
	}
</script>
