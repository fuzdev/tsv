<script lang="ts">
	// own-line comment before a method-signature `;` moves to its own line after `;`
	interface I {
		m(): void;
		/* c1 */
		n(): void;
	}

	// own-line comment before a call- and construct-signature `;` defers past `;`
	type T = {
		(): void;
		/* c2 */
		new (): void;
		/* c3 */
		x: number;
	};
</script>
