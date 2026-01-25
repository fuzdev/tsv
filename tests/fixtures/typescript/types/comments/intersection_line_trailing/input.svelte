<script lang="ts">
	// Line comment trailing first type
	type T = A & // trailing
		B;

	// Line comment trailing multiple types
	type U = A & // trailing A
		B & // trailing B
		C;

	// Trailing on last type before semicolon
	type V = A & B; // trailing
</script>
