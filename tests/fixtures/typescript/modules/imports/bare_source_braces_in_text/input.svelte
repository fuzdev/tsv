<script lang="ts">
	// A side-effect import stays bare when its source string holds braces and `from`;
	// the string is the source, never header syntax
	import '{} from a';
	import '{ } from b';
	import '{/* */} from c';
	import "{} from 'd'";
	import '{}from';
	import '{} from e' with { type: 'json' };
	// the named and re-export siblings, whose braces are real
	import {} from '{} from f';
	export {} from '{} from g';
	export * from '{} from h';
</script>
