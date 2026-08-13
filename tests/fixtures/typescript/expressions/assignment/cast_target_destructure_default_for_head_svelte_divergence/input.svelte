<script lang="ts">
	// A cast target with a default, inside a no-declaration for-of/for-in head.
	for ([(c as T) = 1] of arr) {
	}
	for ([(c satisfies T) = 2] of arr) {
	}
	for ({ a: (b as T) = 3 } of arr) {
	}
	for ([(c as T) = 4] in obj) {
	}
</script>
