<script>
	// `export *` host
	export * from './a' with {type:'json'}/* c */;

	// `export * as ns` host
	export * as ns from './b' with {type:'json'}// 1
	;

	// `export { … } from` host
	export {a} from './c' with {type:'json'}// 2
	;
</script>
