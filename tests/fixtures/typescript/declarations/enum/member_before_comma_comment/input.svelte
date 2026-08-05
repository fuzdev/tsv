<script lang="ts">
	// an own-line line comment before the comma leads the next member
	enum E1 {
		A,
		// c1
		B
	}

	// an own-line block comment before the comma leads the next member too
	enum E2 {
		A,
		/* c1 */
		B
	}

	// a same-line comment trails the member, an own-line one after it leads the next
	enum E3 {
		A, // c1
		// c2
		B
	}

	// a blank line the author left before the own-line comment is preserved
	enum E4 {
		A,

		// c1
		B
	}
</script>
