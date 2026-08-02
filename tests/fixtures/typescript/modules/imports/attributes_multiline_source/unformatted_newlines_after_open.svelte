<script lang="ts">
	/* 1. authored inline - stays inline */
	import a from './a' with {alpha: 'one', beta: 'two'};
	/* 2. newline after `with {` - stays expanded even though it fits */
	import b from './b' with {
alpha: 'one', beta: 'two'};
	/* 3. lone non-`type` attribute, newline after `{` - stays expanded */
	import c from './c' with {
alpha: 'one'};
	/* 4. lone non-`type` attribute, authored inline - stays inline */
	import d from './d' with {alpha: 'one'};
	/* 5. lone `type` attribute collapses anyway - the never-break rule outranks the authored break */
	import e from './e' with {
type: 'json'};
	/* 6. newline only before `}` - collapses to inline */
	import f from './f' with {alpha: 'one', beta: 'two'
};
	/* 7. re-export host - same rule */
	export { gg } from './g' with {
alpha: 'one', beta: 'two'};
	/* 8. export-all host - same rule */
	export * from './h' with {
alpha: 'one', beta: 'two'};
</script>
