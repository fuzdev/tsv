<script lang="ts">
	// Short nested generic stays inline (no boundary needed)
	type Short = Outer<Inner<Entity, 'a'>> & {extra?: string};

	// Intersection member, generic first line exactly 100 - generic stays inline, object breaks
	type IntFit = Outer<Inner<Entity, 'aaaaaa' | 'bbbbbb' | 'cccccc' | 'dddddd' | 'eeeeeeeeeeee'>> & {
		extra?: string;
	};

	// Intersection member, generic first line 101 - breaks the OUTER generic (inner stays inline)
	type IntBrk = Outer<
		Inner<Entity, 'aaaaaa' | 'bbbbbb' | 'cccccc' | 'dddddd' | 'eeeeeeeeeeeee'>
	> & {extra?: string};

	// Union member, generic at exactly 100 - generic stays inline
	type UniFit =
		| Outer<Inner<Entity, 'aaaaaa' | 'bbbbbb' | 'cccccc' | 'dddddd' | 'eeeeeeeeeeeeeeeeeeeeeeeeee'>>
		| {x?: string};

	// Union member, generic at 101 - breaks the OUTER generic (inner stays inline)
	type UniBrk =
		| Outer<
				Inner<Entity, 'aaaaaa' | 'bbbbbb' | 'cccccc' | 'dddddd' | 'eeeeeeeeeeeeeeeeeeeeeeeeeee'>
		  >
		| {x?: string};
</script>
