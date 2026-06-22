<script lang="ts">
	type A = B & C & { c: boolean };

	type D = B & C & D & { d: Array<string> };

	type E = Obj & { e: string };

	// 100 chars - stays flat
	type LongType = SomeLongTypeName &
		AnotherLongTypeName & { propWithLongNameHere: VeryLongTypeName };

	// 101 chars - breaks
	type LongType1 = SomeLongTypeName &
		AnotherLongTypeName & { propWithLongNameHere: VeryLongTypeName };
</script>
