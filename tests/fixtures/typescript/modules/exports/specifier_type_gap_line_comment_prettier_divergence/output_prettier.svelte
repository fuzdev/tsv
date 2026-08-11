<script lang="ts">
	export {
		//c
		type A
	};
	export {
		//c1
		//c2
		type B as C
	} from 'y';
	export { type /* c3 */ D };
</script>
