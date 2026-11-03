<script lang="ts">
	// the interface and class twins of a frozen member whose separator sits below a line comment
	interface I {
		// prettier-ignore
		a:  1 // c
		;
		// prettier-ignore
		m(  ): void // c
		;
		b: 1;
	}
	class K {
		// prettier-ignore
		a  = 1 // c
		;
		b = 1;
	}
</script>
