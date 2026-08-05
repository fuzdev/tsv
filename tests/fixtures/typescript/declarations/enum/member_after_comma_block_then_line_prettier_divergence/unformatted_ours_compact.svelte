<script lang="ts">
	// a block after the comma stays on the comma line, in front of the line comment the
	// author wrote after it
	enum E1 { A, /* c1 */ // c2
B }

	// a run of blocks after the comma keeps its order
	enum E2 { A, /* c1 */ /* c2 */ // c3
B }

	// blocks on both sides of the comma each keep their side of it
	enum E3 { A /* c0 */, /* c1 */ // c2
B }
</script>
