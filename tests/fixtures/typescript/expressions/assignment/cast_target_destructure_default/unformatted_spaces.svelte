<script lang="ts">
	// A cast target with a default in a destructuring pattern KEEPS its assertion
	// wrapper (the parens are grouping and drop out), like acorn's toAssignable.
	({ a: ( b as T ) = 1 } = obj);
	({ a: ( b satisfies T )   =   2 } = obj);
	({ a: (<T> b) = 3 } = obj);
	({ a: b! =   4 } = obj);
	[ ( c as T ) = 5 ] = arr;
</script>
