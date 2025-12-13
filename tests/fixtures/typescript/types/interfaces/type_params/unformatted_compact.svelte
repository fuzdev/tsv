<script lang="ts">
	interface Box<T>{value:T;}

	interface Pair<A,B>{first:A;second:B;}

	interface Constrained<T extends string>{value:T;}

	interface WithDefault<T=string>{value:T;}

	interface Complex<T extends string|number>{value:T;}
</script>
