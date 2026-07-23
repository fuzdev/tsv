<script lang="ts">
	// An astral identifier (a valid ES2015 ID_Start) is a bare object key.
	const obj = { 𐊧: 1 };
	// A type-literal member unquotes the same key.
	type T = { 𐊧: number };
	// An interface member does too.
	interface I {
		𐊧: number;
	}
	// A class field key unquotes the same way; prettier keeps class field keys quoted.
	class C {
		𐊧 = 1;
	}
	// A key that is not a valid identifier stays quoted.
	const digit = { '0a': 2 };
</script>
