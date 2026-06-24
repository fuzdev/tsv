<script lang="ts">
	// Multi-arg type-arg list fits at exactly 100 - stays inline
	type Ra = Map<Keeeeeeeeeeeeeeeeeeeeeeeeeeeeee, { a: number | string; b: number | string | null }>;

	// One char over (101) - the list breaks; each member fits its own line and stays inline
	type Rb = Map<Keeeeeeeeeeeeeeeeeeeeeeeeeeeeeee, { a: number | string; b: number | string | null }>;

	// Wide object member: the list breaks AND the object expands block-style, not the inner union
	type Rc = Map<Key, { aaaaaaaa: number | string; bbbbbbbb: number | string | null; cccccccc: number | string | undefined }>;

	// Annotation position - same behavior
	let v: Map<Key, { aaaaaaaa: number | string; bbbbbbbb: number | string | null; cccccccc: number | string | undefined }>;
</script>
