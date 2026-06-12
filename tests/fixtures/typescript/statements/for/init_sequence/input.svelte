<script lang="ts">
	// For loop with assignment sequence in init
	for (i = 0, j = 0; i < 10; i++, j++) {}

	// For loop with mixed sequence
	for (a, i = 0, b; i < 10; i++) {}

	// For loop with only assignments
	for (x = 0, y = 1; x < y; x++) {}

	// Nested for with sequences
	for (i = 0, j = 0; i < 10; i++) {
		for (x = 0, y = 0; x < 10; x++) {}
	}
</script>
