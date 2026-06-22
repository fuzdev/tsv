<script lang="ts">
	// Short — superclass call stays inline
	class Short extends createBase(a, b) {}

	// At 100 chars — stays inline
	class Fits extends createBase(aaaaaaaa, bbbbbbbb, cccccccc, dddddddd, eeeeeeeeeeeeeeeeeeeeeeee) {}

	// At 101 chars — superclass call arguments break onto their own lines
	class Overr extends createBase(
		aaaaaaaa,
		bbbbbbbb,
		cccccccc,
		dddddddd,
		eeeeeeeeeeeeeeeeeeeeeeee
	) {}

	// Class expression — same superclass call breaks identically
	const G = class extends createBase(
		aaaaaaaa,
		bbbbbbbb,
		cccccccc,
		dddddddd,
		eeeeeeeeeeeeeeeeeeeeeeee
	) {};
</script>
