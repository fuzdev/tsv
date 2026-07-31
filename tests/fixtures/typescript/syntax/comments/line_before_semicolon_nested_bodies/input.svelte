<script lang="ts">
	// An own-line comment before a statement's `;` drops to its own line after it, and a
	// block comment glued to the next statement stays with that statement — the same rule
	// at every nested statement list, not just the program body.

	// Function body.
	function fn() {
		const a = 1;
		// c
		/* b */ ({});
	}

	// Bare block.
	{
		const a = 1;
		// d
		/* e */ ({});
	}

	// Switch case.
	switch (cond) {
		case 1:
			const a = 1;
			// g
			/* h */ ({});
	}

	// Class body.
	class C {
		a = 1;
		// i
		/* j */ b = 2;
	}
</script>
