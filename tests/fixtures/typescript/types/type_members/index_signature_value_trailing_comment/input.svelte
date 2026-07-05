<script lang="ts">
	// same-line block stays before the index-signature `;`
	type A = {
		[k: string]: number /* c1 */;
	};

	// own-line comment defers past the index-signature `;`
	interface B {
		[k: string]: number;
		/* c2 */
	}

	// class index signature: same-line block stays before `;`, same-line line trails past it
	class C {
		[k: string]: number /* c3 */;
		[j: number]: string; // c4
	}
</script>
