<script lang="ts">
	// a directive written in an EMPTY clause slot stays in that slot, so it freezes
	// nothing — the clause it would freeze is on the other side of the `;`
	for (
		;
		// prettier-ignore
		b < 10;
		c++
	) {
		fn();
	}
</script>
