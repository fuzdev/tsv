<script lang="ts">
	// Short case - exactly 100 chars, does not wrap
	const a: Map<LongTypeNameHereAbcdefghijklmn, AnotherLongTypeNameXyzabcde> = fn(new Map(this.arg));

	// Long case with new - outer call breaks, inner stays together (101 chars)
	const b: Map<LongTypeNameHereAbcdefgh, AnotherLongTypeNameXyzabc> = fn(
		new Map(this.aaaaaaaaaaaa),
	);

	// Long case with call - outer call breaks, inner stays together (101 chars)
	const c: Map<LongTypeNameHereAbcdefghi, AnotherLongTypeNameXyzabc> = fn(
		create(this.aaaaaaaaaaaa),
	);

	// Class field with rune (real-world pattern)
	class A {
		readonly d: Map<LongTypeNameHereAbcdefgh, AnotherLongTypeNameXyz> = $derived(
			new Map(this.aaaa),
		);
	}

	// Nested new expressions - outer breaks, innermost stays together
	const e: Map<LongTypeNameHereAbcdefgh, AnotherLongTypeNameXyzabc> = fn(
		new Map(new Set(this.aaaa)),
	);
</script>
