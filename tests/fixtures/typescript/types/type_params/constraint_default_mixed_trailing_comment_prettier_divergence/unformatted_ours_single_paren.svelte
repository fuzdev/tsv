<script lang="ts">
	type D<U = (/* b */ // c
		A)> = U;
	type E<U = (// c
		B /* t */)> = U;
	type F<U extends (/* b */ // c
		A)> = U;
	type G<U extends (// c
		B /* t */)> = U;
</script>