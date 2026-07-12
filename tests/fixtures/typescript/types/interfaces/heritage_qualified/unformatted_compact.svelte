<script lang="ts">
	// Qualified `extends`: the heritage `expression` is a `TSQualifiedName`
	interface Single extends ns.Base{}

	// Deeply nested qualified `extends`
	interface Nested extends a.b.Base{}

	// Qualified `implements`: the heritage `expression` is a `TSQualifiedName` too
	class SingleImpl implements ns.Contract{}

	// Deeply nested qualified `implements`
	class NestedImpl implements a.b.Contract{}
</script>
