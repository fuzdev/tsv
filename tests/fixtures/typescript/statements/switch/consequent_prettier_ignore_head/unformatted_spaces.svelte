<script lang="ts">
	// an own-line directive in the label→first-statement gap freezes that statement
	switch (  aaa  ) {
		case   1:
			// prettier-ignore
			fn(  bbb  );
			fn(   ccc   );
	}

	// between consequent statements, the directive freezes the one that follows it
	switch (   aaa   ) {
		case    1:
			fn(   bbb   );
			// prettier-ignore
			fn(  ccc  );
	}

	// a blank line above the directive is authoring intent and is preserved
	switch (aaa) {
		case  1:
			fn(    bbb    );

			// prettier-ignore
			fn(  ccc  );
	}

	// an own-line block comment behaves identically — placement keys the freeze
	switch (  aaa  ) {
		case   1:
			/* prettier-ignore */
			fn(  bbb  );
	}
</script>
