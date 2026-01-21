<script lang="ts">
	// Line comment trailing first parameter
	type T = (a: A,// trailing
		b: B) => void;

	// Line comment trailing multiple parameters
	type U = (a: A,// trailing A
		b: B,// trailing B
		c: C) => void;

	// Trailing on last parameter
	type V = (a: A,b: B// trailing
	) => void;
</script>
