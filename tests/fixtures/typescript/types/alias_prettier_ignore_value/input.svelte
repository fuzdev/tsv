<script lang="ts">
	// an own-line directive before the alias RHS freezes the whole value
	type A =
		// prettier-ignore
		{x:   1};

	// function-type value
	type B =
		// prettier-ignore
		(a:   1) => void;

	// own-line block spelling behaves like the line spelling
	type C =
		/* prettier-ignore */
		{x:   1};

	// a multi-line value keeps its verbatim layout
	type D =
		// prettier-ignore
		{
			x:   1;
				y: 2;
		};
</script>
