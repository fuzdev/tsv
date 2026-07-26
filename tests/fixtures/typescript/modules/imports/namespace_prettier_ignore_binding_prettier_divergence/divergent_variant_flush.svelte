<script lang="ts">
	// an own-line directive after the keyword freezes the whole namespace binding and
	// keeps its own line
	import // prettier-ignore
	*   as   ns1 from './a';

	// with another comment already in the gap the directive still gets its own line
	import // c
	// prettier-ignore
	*   as   ns2 from './b';

	// a default binding has no interior whitespace to preserve, but the directive's
	// own line is the same placement rule
	import // prettier-ignore
	d1 from './c';
</script>
