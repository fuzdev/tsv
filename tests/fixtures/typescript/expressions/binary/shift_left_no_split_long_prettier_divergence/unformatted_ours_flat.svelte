<script lang="ts">
	// the space between the two `<` counts toward the width: a line it brings to exactly 100
	// columns stays flat, and one it brings to 101 breaks after the `<`, where no space is owed
	type AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA = typeof f< <T>(v: T) => R>;
	type AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA = typeof f< <T>(v: T) => R>;
	class BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB extends fn< <T>(v: T) => R> {}
	class BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB extends fn< <T>(v: T) => R> {}

	// an assertion first wraps its operand, its cast still flat and still spaced, and breaks
	// the cast one column later
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = < <T>(v: T) => R>x;
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = < <T>(v: T) => R>x;
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = < <T>(v: T) => R>x;
</script>
