<script lang="ts">
	// a reserved word after the `type` modifier names another module's export
	export {type default} from './a';
	export {type default as b} from './b';
	export {type default as 'c'} from './c';
	export {type if} from './d';
	export {type class as E} from './e';
	export {type typeof as default} from './f';
	export {g, type default as h, type with} from './g';
	export {type if as as} from './h';
	export {type enum, type null, type await} from './i';
</script>
