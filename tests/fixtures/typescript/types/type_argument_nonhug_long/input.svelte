<script lang="ts">
	// A single type argument inlines only when it HUGS (prettier's `shouldHugType`): a
	// simple type, an object type, or a hugged union. A simple type arg stays inline:
	declare const simple: typeof fn<AaaaTypeReference>;

	// An object type arg hugs the `<` and expands block-style inside it:
	declare const obj: typeof fn<{
		aaaaaaaaaaaaaaaaaaaaaaaaaa: Aaaaaaaaaaaaaaaaaaaa;
		bbbbbbbbbb: Bbbbbb;
	}>;

	// A union with an object member hugs too - the object expands, the `<` stays glued:
	declare const uni: typeof fn<{
		aaaaaaaaaaaaaaaaaaaaaaaaaa: Aaaaaaaaaaaaaaaaaaaa;
		bbbbbb: Bbbbbb;
	} | null>;

	// 100 chars - an intersection type arg does not hug, but fits inline (at boundary)
	declare const t100: typeof fn<AAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCCCCCC>;

	// 101 chars - does not fit, and an intersection is not huggable, so the `<...>` breaks
	declare const t101: typeof fn<
		AAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCCCCCC
	>;

	// A function type arg does not hug - the `<...>` breaks
	declare const fnType: typeof fn<
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: AAAAAAAAAAAAAAAAAAAAAAAA) => BBBBBBBBBBBB
	>;

	// A conditional type arg does not hug - the `<...>` breaks
	declare const cond: typeof fn<
		AAAAAAAAAAAAAAAAAAAAAAAA extends BBBBBBBBBBBBBBBB ? CCCCCCCCCCCC : DDDDDDDDDD
	>;
</script>
