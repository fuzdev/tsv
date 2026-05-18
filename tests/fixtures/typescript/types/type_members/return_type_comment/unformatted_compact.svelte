<script lang="ts">
	// Interface method return type with comment
	interface A {
		method():/* c */B|C;
	}

	// Interface call signature return type with comment
	interface D {
		():/* c */B|C;
	}

	// Interface construct signature return type with comment
	interface E {
		new():/* c */B|C;
	}

	// Declare function return type with comment
	declare function fn1():/* c */B|C;
</script>
