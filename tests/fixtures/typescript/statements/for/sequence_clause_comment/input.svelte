<script lang="ts">
	// block comment inside for-init/update sequence clauses — stays inline
	for (a = 1, /* c1 */ b = 2; a < 10; a++, /* c2 */ b++) {}

	// line comment inside a for-init sequence clause — forces the header to break
	for (
		e = 1, // c3
			f = 2;
		e < 10;
		e++
	) {}

	// line comment inside a for-update sequence clause — forces the header to break
	for (
		g = 1;
		g < 10;
		g++, // c4
			h++
	) {}
</script>
