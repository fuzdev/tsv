<script lang="ts">
	// An astral identifier (a valid ES2015 ID_Start) is a bare object key.
	const obj = { 𐊧: 1 };
	// A type-literal member unquotes the same key.
	type T = { 𐊧: number };
	// An interface member does too.
	interface I {
		𐊧: number;
	}
	// A class method key unquotes the same way.
	class C {
		𐊧() {}
	}
	// An enum member name unquotes the same way.
	enum E {
		𐊧 = 1
	}
	// A key that is not a valid identifier stays quoted.
	const digit = { '0a': 2 };
</script>
