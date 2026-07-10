<script lang="ts">
	// An intersection with an expanding FIRST member (object) and 3+ members, in a
	// type-annotation position. The object stays inline and hugs the first
	// successor (`} & Bbbb &`); only further successors break to their own line —
	// matching the type-alias RHS and prettier's per-member indent for object-to-
	// non-object transitions.

	// Variable annotation
	let v: { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: 1 } & Bbbbbbbbbbbbbbbbbbbbbbbbbbbb &
		Cccccccccccccccccccccccccccc;

	// Class property
	class C {
		p: { aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: 1 } & Bbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Cccccccccccccccccccccccccccc;
	}
</script>
