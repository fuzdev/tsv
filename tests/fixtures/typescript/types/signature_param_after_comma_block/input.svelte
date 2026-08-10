<script lang="ts">
	// A block hugging the next param leads it, on the far side of the comma the author
	// wrote it after
	interface A {
		m(a: string, /* c */ b: number): void;
	}

	// A block before the comma trails the previous param, on its own side
	interface B {
		m(a: string /* c */, b: number): void;
	}

	// Both sides in one gap, each kept where it was written
	interface C {
		m(a: string /* c1 */, /* c2 */ b: number): void;
	}

	// A construct signature takes the same rule
	interface D {
		new (a: string, /* c */ b: number): void;
	}

	// So does a bodyless declared function
	declare function fn(a: string, /* c */ b: number): void;
	// A call signature, and the comma the author pushed onto the comment's line: the block
	// still leads the next param
	interface E {
		<T, U>(x: T, /* c */ y: U): [T, U];
	}
</script>
