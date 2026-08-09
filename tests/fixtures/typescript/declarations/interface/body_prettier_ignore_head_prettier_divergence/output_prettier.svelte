<script lang="ts">
	// an own-line directive in the header→`{` gap freezes the whole interface body
	interface Aaa {
		// prettier-ignore
		mmm   :   1;
	}

	// a heritage clause is parent-owned and normalizes; the body still freezes
	interface Bbb extends Aaa {
		// prettier-ignore
		nnn   :   2;
	}

	// an own-line block comment behaves identically — placement keys the freeze
	interface Ccc {
		/* prettier-ignore */
		ooo   :   3;
	}
</script>
