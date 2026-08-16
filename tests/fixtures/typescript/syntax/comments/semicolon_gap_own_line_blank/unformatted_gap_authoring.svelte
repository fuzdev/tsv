<script lang="ts">
	// A blank line the author left above an own-line comment in a statement's
	// content→`;` gap survives the terminator — whichever side of the `;` a
	// same-line block on that content's own line would bind to.
	import a = B.C

	/* a */;

	export as namespace D

	/* d */;

	class F {
		g = 1

		/* g */;
	}

	const e = 1

	/* e */;
</script>
