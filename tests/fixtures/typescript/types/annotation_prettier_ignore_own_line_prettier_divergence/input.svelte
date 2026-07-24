<script lang="ts">
	// an own-line directive after the annotation's `:` stays own-line and freezes the
	// type that follows it
	let v:
		// prettier-ignore
		{x:   1};

	// return-type host
	function fn():
		// prettier-ignore
		{x:   1} {
		return { x: 1 };
	}

	// property-signature host
	interface I {
		a:
			// prettier-ignore
			{x:   1};
	}

	// a multi-line type keeps its verbatim layout
	let w:
		// prettier-ignore
		{
			x:   1;
				y: 2;
		};
</script>
