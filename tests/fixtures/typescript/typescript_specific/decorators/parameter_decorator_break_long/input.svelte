<script lang="ts">
	// Once the parameter list breaks, a decorator and its binding are one group
	// joined by a `line`: the decorator keeps the binding's line while the pair
	// fits, and drops to its own line when it does not.

	class Boundary {
		// The pair is exactly 100 wide, so the decorator stays on the binding's line
		fits100(
			@dec aaa: SomeLongTypeName<Aaaaaaaaaaaa, Bbbbbbbbbbbb, Cccccccccccc, Ddddddddddddddddddddddd>,
			b: Bb
		) {}
		// One char more and the pair breaks — the decorator takes its own line
		breaks101(
			@dec
			aaa: SomeLongTypeName<Aaaaaaaaaaaa, Bbbbbbbbbbbb, Cccccccccccc, Dddddddddddddddddddddddd>,
			b: Bb
		) {}

		// A sole parameter: the decorator's own line, then the object expands
		sole(
			@dec
			a: {
				aaa: Aaaa;
				bbb: Bbbb;
				ccc: Cccc;
				ddd: Dddd;
				eee: Eeee;
				fff: Ffff;
				ggg: Gggg;
				hhh: Hhhh;
				iii: Iiii;
			}
		) {}

		// A parameter property takes the same group
		constructor(
			@Inject(TOKEN)
			private readonly aaaaaa: SomeLongTypeName<Aaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb>
		) {}
	}
</script>
