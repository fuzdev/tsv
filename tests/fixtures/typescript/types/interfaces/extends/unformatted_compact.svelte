<script lang="ts">
	interface Single extends Base{prop:string;}

	interface Multiple extends Base,Other{prop:string;}

	interface WithGenerics<T> extends Base<T>{value:T;}

	interface MultipleWithGenerics<T> extends Base<T>,Other<T>{value:T;}

	interface Empty extends Base{}
</script>
