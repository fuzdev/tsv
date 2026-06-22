<script lang="ts">
	// import: same name preserved (not stripped)
	import { a as a } from 'a';

	// import: same name with block comment
	import { b as /* c */ b } from 'b';

	// import: multiple specifiers, one same-name
	import { c, d as d } from 'c';

	// import: type specifier same name
	import { type A as A } from 'd';

	// export: same name preserved
	export { a as a };

	// export: same name with block comment
	export { b as /* c */ b };

	// export: re-export same name
	export { c as c } from 'e';

	// export: re-export same name with block comment
	export { d as /* c */ d } from 'f';
</script>
