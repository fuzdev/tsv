<script lang="ts">
	// A block the author stranded on the comma's line, with the next param below it, keeps
	// the comma's line - a method signature
	interface A {
		m(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong1: string, /* c */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong2: number
		): void;
	}

	// a construct signature
	interface B {
		new (
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong1: string, /* c */
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong2: number
		): void;
	}

	// and a bodyless declared function
	declare function fn(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong1: string, /* c */
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaLong2: number
	): void;
</script>
