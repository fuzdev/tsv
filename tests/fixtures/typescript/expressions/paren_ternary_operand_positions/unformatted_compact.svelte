<script lang="ts">
	// A call argument is not one of the value positions that expand the parens:
	// the `?`/`:` arms hang under the `(` instead
	fnnn((cccccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as Foo);

	// An array element hangs the same way
	const a = [(cccccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as Foo];

	// A property value hangs the same way
	const b = { kkk: (cccccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as Foo };

	// A non-null assertion in a call argument hangs too
	fnnn((cccccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)!);

	// Member base in a call argument: arms hang, and the `)` drops to its own line
	fnnn((cccccccccccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb).prop);

	// With a non-null between base and member the `)` stays welded to the last arm
	fnnn((cccccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)!.prop);
</script>
