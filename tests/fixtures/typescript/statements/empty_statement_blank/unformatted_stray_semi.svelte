<script lang="ts">
	// A blank before a dropped `;` belongs to the statement above it.
	a1();

	;
	b1();

	// A blank after a dropped `;` sits in a gap the printer does not reproduce.
	a2();
	;

	b2();

	// A blank on both sides collapses to the one the statement above owns.
	a3();

	;

	b3();

	// Block body, same rule.
	function fn() {
		a4();

		;
		b4();
	}

	// Switch case consequent, same rule.
	switch (cond) {
		case 1:
			a5();

			;
			b5();
	}

	// Namespace body, same rule.
	namespace N {
		a6();

		;
		b6();
	}
</script>
