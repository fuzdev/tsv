<script lang="ts">
	// A stranded after-comma block (the author left a newline before the next
	// param) stays trailing the comma; prettier relocates it before the comma
	type Fn = (
		argumentnumberoneeeeee, /* comment */
		argumentnumbertwooooooooooooooooooooooooooo
	) => void;

	// A before-comma block and a stranded after-comma block in the same gap each
	// stay on their own side of the comma; prettier relocates both before it
	type Combo = (
		argumentnumberoneeeeee /* c1 */, /* c2 */
		argumentnumbertwoooooooooooooooooooooooo
	) => void;

	// Constructor type: same rule
	type Ctor = new (
		argumentnumberoneeeeee, /* comment */
		argumentnumbertwoooooooooooooooooooooo
	) => Type;
</script>
