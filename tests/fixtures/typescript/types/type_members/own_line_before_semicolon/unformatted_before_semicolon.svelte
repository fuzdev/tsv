<script lang="ts">
	// own-line block comment before a type-literal property `;` moves to its own line after `;`
	type A = {
		a: number
		/* c1 */
		;
		b: string;
	};

	// a same-line block stays before `;`; a following own-line block defers after `;`
	type C = {
		a: number /* c2 */
		/* c3 */
		;
		b: string;
	};

	// own-line line comment before an interface property `;` moves to its own line after `;`,
	// and a blank line before it is preserved
	interface B {
		c: number

		// c4
		;
		d: string;
	}
</script>
